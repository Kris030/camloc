use anyhow::{anyhow, Result};
use camloc_common::yes_no_choice;
use camloc_server::{
    compass::Compass,
    extrapolations::{Extrapolation, LinearExtrapolation},
    service::{LocationService, Subscriber},
    PlacedCamera, TimedPosition,
};
use std::{net::SocketAddr, time::Duration};
use tokio::io::{stderr, AsyncWriteExt};

fn main() {
    if let Err(e) = run() {
        println!("Exiting with error: {e}");
    } else {
        println!("Exiting test...");
    }
}

#[cfg(feature = "serial-compass")]
fn get_compass() -> Result<Option<Box<dyn Compass + Send>>> {
    use camloc_common::get_from_stdin;
    use camloc_server::compass::serial::SerialCompass;
    use tokio_serial::{SerialPortBuilderExt, SerialPortType};

    if !yes_no_choice("Do you want to use a microbit compass?", false) {
        return Ok(None);
    }

    let Ok(devices) = tokio_serial::available_ports() else {
        println!("  Couldn't get available serial devices");
        return Ok(None);
    };
    if devices.is_empty() {
        println!("  No serial devices available");
        return Ok(None);
    }

    println!("  Available serial devices:");

    for (i, d) in devices.iter().enumerate() {
        println!(
            "  {i:<3}{} | {}",
            d.port_name,
            match &d.port_type {
                SerialPortType::BluetoothPort => "Bluetooth".to_string(),
                SerialPortType::Unknown => "unknown".to_string(),
                SerialPortType::UsbPort(info) => {
                    let mut s = "USB".to_string();
                    if let Some(m) = &info.manufacturer {
                        s.push_str(" | ");
                        s.push_str(m);
                    }
                    if let Some(m) = &info.product {
                        s.push_str(" | ");
                        s.push_str(m);
                    }

                    s
                }
                SerialPortType::PciPort => "PCI".to_string(),
            }
        );
    }

    let d = &devices[get_from_stdin::<usize>("  Enter index: ")?];
    let baud_rate = get_from_stdin("  Enter baud rate (115200hz): ").unwrap_or(115200);
    let offset = get_from_stdin("  Enter compass offset in degrees (0 deg): ").unwrap_or(0u8);

    let p = tokio_serial::new(&d.port_name, baud_rate)
        .open_native_async()
        .map(|p| SerialCompass::start(p, offset as f64));

    if let Ok(Ok(p)) = p {
        Ok(Some(Box::new(p)))
    } else {
        Err(anyhow!("Couldn't open serial port"))
    }
}

#[tokio::main]
async fn run() -> Result<()> {
    let location_service = LocationService::start(
        Some(Extrapolation::new::<LinearExtrapolation>(
            Duration::from_millis(500),
        )),
        // no_extrapolation!(),
        camloc_common::hosts::constants::MAIN_PORT,
        #[cfg(feature = "serial-compass")]
        get_compass()?,
        #[cfg(not(feature = "serial-compass"))]
        camloc_server::compass::no_compass!(),
        Duration::from_millis(500),
    )
    .await?;

    location_service
        .subscribe(Subscriber::Connection(|address, camera| {
            let address = address.to_string();
            println!("New camera connected from {address}");
            Box::pin(async move { Ok(on_connect(address, camera).await?) })
        }))
        .await;

    location_service
        .subscribe(Subscriber::Disconnection(|c, _| {
            Box::pin(async move { Ok(on_disconnect(c).await?) })
        }))
        .await;

    let ctrlc_task = tokio::spawn(async move { tokio::signal::ctrl_c().await });

    if yes_no_choice("Subscription or query mode?", true) {
        location_service
            .subscribe(Subscriber::Position(|p| {
                Box::pin(async move { Ok(on_position(p).await?) })
            }))
            .await;
    } else {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        loop {
            if let Some(p) = location_service.get_position().await {
                on_position(p).await?;
            } else {
                println!("Couldn't get position");
            }

            if ctrlc_task.is_finished() {
                break;
            }

            interval.tick().await;
        }
    }

    if let Err(_) | Ok(Err(_)) = ctrlc_task.await {
        return Err(anyhow!("Something failed in the ctrl+c channel"));
    }

    Ok(())
}

async fn on_position(p: TimedPosition) -> tokio::io::Result<()> {
    println!("{p}");

    let mut buf = 0i32.to_be_bytes().to_vec();
    buf.append(
        &mut [
            p.position.x.to_be_bytes(),
            p.position.y.to_be_bytes(),
            p.position.rotation.to_be_bytes(),
        ]
        .concat(),
    );

    stderr().write_all(&buf[..]).await?;

    Ok(())
}

async fn on_connect(address: String, camera: PlacedCamera) -> tokio::io::Result<()> {
    let mut se = stderr();
    se.write_i32(1).await?;
    se.write_u16(address.len() as u16).await?;
    se.write_all(address.as_bytes()).await?;

    se.write_f64(camera.position.x).await?;
    se.write_f64(camera.position.y).await?;
    se.write_f64(camera.position.rotation).await?;

    se.write_f64(camera.fov).await?;

    Ok(())
}

async fn on_disconnect(address: SocketAddr) -> tokio::io::Result<()> {
    let address = address.to_string();
    println!("Camera disconnected from {address}");
    let mut se = stderr();
    se.write_i32(2).await?;
    se.write_u16(address.len() as u16).await?;
    se.write_all(address.as_bytes()).await?;
    Ok(())
}

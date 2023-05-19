use camloc_common::get_from_stdin;
use camloc_server::{
    calc::PlacedCamera,
    compass::serial::SerialCompass,
    extrapolations::{Extrapolation, LinearExtrapolation},
    service::{LocationService, Subscriber, TimedPosition},
};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::{
    io::{stderr, AsyncWriteExt},
    sync::watch,
};
use tokio_serial::{SerialPortBuilderExt, SerialPortType};

fn main() {
    if let Err(e) = run() {
        println!("[ERROR]: {e}");
    } else {
        println!("No errors");
    }
    println!("Exiting test...");
}

async fn send_camera(address: String, camera: PlacedCamera) -> tokio::io::Result<()> {
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

fn get_compass() -> Result<Option<SerialCompass>, &'static str> {
    let yes: String = get_from_stdin("Do you want to use a microbit compass? (y/N) ")?;
    if !matches!(&yes[..], "y" | "Y") {
        return Ok(None);
    }

    let devices = if let Ok(ps) = tokio_serial::available_ports() {
        ps
    } else {
        println!("  Couldn't get available serial devices");
        return Ok(None);
    };
    if devices.is_empty() {
        println!("No serial devices available");
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
    let offset = get_from_stdin("  Enter compass offset (degrees): ")?;

    let p = tokio_serial::new(&d.port_name, baud_rate)
        .open_native_async()
        .map(|p| SerialCompass::start(p, offset));

    if let Ok(Ok(p)) = p {
        Ok(Some(p))
    } else {
        Err("Couldn't open serial port")
    }
}

#[tokio::main]
async fn run() -> Result<(), String> {
    let compass = Box::leak(get_compass()?.into());
    let mut location_service = LocationService::start(
        Some(Extrapolation::<LinearExtrapolation>::new(
            Duration::from_millis(500),
        )),
        camloc_common::hosts::constants::MAIN_PORT,
        compass
            .as_mut()
            .map(|compass| || async { compass.get_value().await }),
        // no_compass!(),
    )
    .await?;

    location_service
        .subscribe(Subscriber::Connection(|address, camera| {
            let address = address.to_string();
            println!("New camera connected from {address}");
            Box::pin(async move {
                send_camera(address, camera).await.unwrap();
            })
        }))
        .await;

    location_service
        .subscribe(Subscriber::Disconnection(|address, _| {
            let address = address.to_string();
            println!("Camera disconnected from {address}");
            Box::pin(async move {
                let mut se = stderr();
                se.write_i32(2).await.unwrap();
                se.write_u16(address.len() as u16).await.unwrap();
                se.write_all(address.as_bytes()).await.unwrap();
            })
        }))
        .await;

    fn write_to_stderr_binary(p: TimedPosition) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        use tokio::io::{stderr, AsyncWriteExt};
        Box::pin(async move {
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

            stderr()
                .write_all(&buf[..])
                .await
                .expect("Couldn't write coords to stderr???");
        })
    }

    let (tx, mut rx) = watch::channel(());
    // needed because of static lifetime
    let tx = Box::leak(tx.into());
    ctrlc::set_handler(|| {
        if tx.send(()).is_err() {
            println!("ctrlc pressed but unable to handle signal");
        }
    })
    .map_err(|_| "Couldn't setup ctrl+c handler")?;

    if true {
        location_service
            .subscribe(Subscriber::Position(write_to_stderr_binary))
            .await;

        rx.changed()
            .await
            .map_err(|_| "Something failed in the ctrl+c channel")?;
        location_service.stop().await;
    } else {
        loop {
            if let Some(p) = location_service.get_position().await {
                write_to_stderr_binary(p).await;
            } else {
                println!("Couldn't get position");
            }

            if rx
                .has_changed()
                .map_err(|_| "Something failed in the ctrl+c channel")?
            {
                break;
            }
        }
    }

    Ok(())
}

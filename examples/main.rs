use anyhow::{anyhow, Result};
use camloc_common::yes_no_choice;
use camloc_server::{
    extrapolations::LinearExtrapolation,
    service::{self, Event, Subscriber},
    PlacedCamera, TimedPosition, MAIN_PORT,
};

#[cfg(feature = "serial-compass")]
use camloc_server::compass::Compass;

use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{stderr, AsyncWriteExt},
    spawn,
};

fn main() {
    if let Err(e) = run() {
        println!("Exiting with error: {e}");
    } else {
        println!("Exiting test...");
    }
}

#[cfg(feature = "serial-compass")]
async fn get_compass(
    used: &mut std::collections::HashSet<String>,
) -> Result<Option<Box<dyn Compass + Send + 'static>>> {
    use camloc_common::{choice, get_from_stdin};
    use camloc_server::compass::serial::SerialCompass;
    use tokio_serial::{SerialPortBuilderExt, SerialPortType};

    if !yes_no_choice("Do you want to use a serial compass?", false) {
        return Ok(None);
    }

    let mut devices = tokio_serial::available_ports()?;
    devices.retain(|d| !used.contains(&d.port_name));

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
                    if let Some(m) = &info.product {
                        s.push_str(" | product: ");
                        s.push_str(m);
                    }
                    if let Some(m) = &info.manufacturer {
                        s.push_str(" by ");
                        s.push_str(m);
                    }
                    s
                }
                SerialPortType::PciPort => "PCI".to_string(),
            }
        );
    }

    let d = devices.remove(get_from_stdin::<usize>("  Enter index: ")?);
    let baud_rate = get_from_stdin("  Enter baud rate (115200hz): ").unwrap_or(115200);
    let offset = get_from_stdin::<u8>(
        "  Enter compass offset (degrees, can be interactively chosen later): ",
    )
    .ok();

    let name = d.port_name;
    let mut native = tokio_serial::new(&name, baud_rate).open_native_async()?;
    native.set_exclusive(true)?;

    let mut compass = SerialCompass::start(
        native,
        Duration::from_millis(10),
        offset.unwrap_or(0u8).into(),
        name.clone(),
    );

    if offset.is_none() && yes_no_choice("  Do you want to set a new offset?", true) {
        let mut offset = 0f64;
        loop {
            let offset_degrees = format!("{:.2}", offset.to_degrees());
            println!("    Current offset: {offset_degrees}°");

            let Some(value) = compass.get_value().await else {
                println!("    Couldn't get compass value");
                if yes_no_choice("    Do you still want to continue?", false) {
                    continue;
                } else {
                    break;
                }
            };
            let value_degrees = format!("{:.2}", value.to_degrees());
            println!("    Current compass value (without offset): {value_degrees}°");

            let c = choice(
                [
                    (
                        &(format!("Exit and use previous value ({offset_degrees}°)")) as &str,
                        true,
                    ),
                    (
                        &format!("Use current value ({value_degrees}°) as offset and exit"),
                        true,
                    ),
                    ("Get new value", true),
                ]
                .into_iter(),
                None,
                Some(2),
            )?;
            println!("\n");

            match c {
                0 => {
                    compass.set_offset(offset).await;
                    break;
                }

                1 => {
                    compass.set_offset(value).await;
                    break;
                }

                _ => offset = value,
            }
        }
    }

    used.insert(name);

    Ok(Some(Box::new(compass)))
}

#[cfg(feature = "serial-compass")]
async fn get_compasses() -> Result<Vec<Box<dyn Compass + Send + 'static>>> {
    let mut used = std::collections::HashSet::new();
    let mut compasses = vec![];

    while let Some(c) = get_compass(&mut used).await? {
        compasses.push(c);
    }

    Ok(compasses)
}

#[tokio::main]
async fn run() -> Result<()> {
    #[cfg(feature = "serial-compass")]
    let compasses = get_compasses().await?;
    #[cfg(not(feature = "serial-compass"))]
    let compasses = [];

    let location_service = service::start(
        Some(LinearExtrapolation::new()),
        // no_extrapolation!(),
        MAIN_PORT,
        compasses.into_iter(),
        Duration::from_millis(500),
    )
    .await?;

    location_service.subscribe(MySubscriber).await;

    let ctrlc_task = tokio::spawn(tokio::signal::ctrl_c());

    if yes_no_choice("Subscription mode (or query)?", true) {
        // nothing, we'll wait wait for ctr+c
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

    if !matches!(ctrlc_task.await, Ok(Ok(_))) {
        return Err(anyhow!("Something failed in the ctrl+c channel"));
    }

    Ok(())
}

struct MySubscriber;
impl Subscriber for MySubscriber {
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Connect(address, camera) => {
                spawn(on_connect(address, camera));
            }

            Event::Disconnect(address) => {
                spawn(on_disconnect(address));
            }

            Event::InfoUpdate(address, camera) => {
                spawn(on_info_update(address, camera));
            }

            Event::PositionUpdate(position) => {
                spawn(on_position(position));
            }
        }
    }
}

async fn on_position(position: TimedPosition) -> tokio::io::Result<()> {
    println!("{position}");

    let mut se = stderr();
    se.write_i32(0).await?;

    se.write_f64(position.position.x).await?;
    se.write_f64(position.position.y).await?;
    se.write_f64(position.position.rotation).await?;

    Ok(())
}

async fn on_connect(address: SocketAddr, camera: PlacedCamera) -> tokio::io::Result<()> {
    let address = address.to_string();
    println!("New camera connected from {address}");

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

async fn on_info_update(address: SocketAddr, camera: PlacedCamera) -> tokio::io::Result<()> {
    let address = address.to_string();

    let mut se = stderr();
    se.write_i32(3).await?;

    se.write_u16(address.len() as u16).await?;
    se.write_all(address.as_bytes()).await?;

    se.write_f64(camera.position.x).await?;
    se.write_f64(camera.position.y).await?;
    se.write_f64(camera.position.rotation).await?;

    se.write_f64(camera.fov).await?;

    Ok(())
}

use anyhow::Result;
use camloc_common::{yes_no_choice, Position};
use camloc_server::{
    service::{self, Event},
    PlacedCamera,
};

#[cfg(feature = "serial-compass")]
use camloc_server::compass::Compass;
use tokio_util::sync::CancellationToken;

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
async fn get_compass() -> Result<camloc_server::compass::serial::SerialCompass> {
    use camloc_common::{choice, get_from_stdin};
    use camloc_server::compass::serial::SerialCompass;
    use tokio_serial::{SerialPortBuilderExt, SerialPortType};

    let mut devices = tokio_serial::available_ports()?;

    if devices.is_empty() {
        return Err(anyhow::Error::msg("No serial devices available"));
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

    Ok(compass)
}

#[tokio::main]
async fn run() -> Result<()> {
    let service = service::Builder::new();

    #[cfg(feature = "serial-compass")]
    let service = service.with_compass(get_compass().await?);

    let mut service = service.start().await?;

    service.enable_events().await;

    let cancell_parent = CancellationToken::new();
    let cancell = cancell_parent.child_token();
    spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        cancell_parent.cancel();
    });

    if yes_no_choice("Subscription mode (or query)?", true) {
        loop {
            let ev = tokio::select! {
                e = service.get_event() => e,
                _ = cancell.cancelled() => break
            }?;

            match ev {
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
    } else {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        while !cancell.is_cancelled() {
            if let Some(p) = service.get_position().await {
                on_position(p).await?;
            } else {
                println!("Couldn't get position");
            }

            interval.tick().await;
        }
    }

    Ok(())
}

async fn on_position(position: Position) -> tokio::io::Result<()> {
    println!("{position}");

    let mut se = stderr();
    se.write_all(
        &[
            0i32.to_be_bytes().as_slice(),
            position.x.to_be_bytes().as_slice(),
            position.y.to_be_bytes().as_slice(),
            position.rotation.to_be_bytes().as_slice(),
        ]
        .concat(),
    )
    .await?;

    Ok(())
}

async fn on_connect(address: SocketAddr, camera: PlacedCamera) -> tokio::io::Result<()> {
    let address = address.to_string();
    println!("New camera connected from {address}");

    let mut se = stderr();
    se.write_all(
        &[
            1i32.to_be_bytes().as_slice(),
            (address.len() as u16).to_be_bytes().as_slice(),
            address.as_bytes(),
            camera.position.x.to_be_bytes().as_slice(),
            camera.position.y.to_be_bytes().as_slice(),
            camera.position.rotation.to_be_bytes().as_slice(),
            camera.fov.to_be_bytes().as_slice(),
        ]
        .concat(),
    )
    .await?;
    Ok(())
}

async fn on_disconnect(address: SocketAddr) -> tokio::io::Result<()> {
    let address = address.to_string();
    println!("Camera disconnected from {address}");

    let mut se = stderr();

    se.write_all(
        &[
            2i32.to_be_bytes().as_slice(),
            (address.len() as u16).to_be_bytes().as_slice(),
            address.as_bytes(),
        ]
        .concat(),
    )
    .await?;

    Ok(())
}

async fn on_info_update(address: SocketAddr, camera: PlacedCamera) -> tokio::io::Result<()> {
    let address = address.to_string();

    let mut se = stderr();
    se.write_all(
        &[
            3i32.to_be_bytes().as_slice(),
            (address.len() as u16).to_be_bytes().as_slice(),
            address.as_bytes(),
            camera.position.x.to_be_bytes().as_slice(),
            camera.position.y.to_be_bytes().as_slice(),
            camera.position.rotation.to_be_bytes().as_slice(),
            camera.fov.to_be_bytes().as_slice(),
        ]
        .concat(),
    )
    .await?;

    Ok(())
}

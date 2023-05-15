use camloc_server::{
    calc::PlacedCamera,
    extrapolations::{Extrapolation, LinearExtrapolation},
    service::{LocationService, Subscriber, TimedPosition},
};
use std::{future::Future, pin::Pin, time::Duration};
use tokio::sync::watch;

fn main() {
    if let Err(e) = run() {
        println!("[ERROR]: {e}");
    } else {
        println!("No errors");
    }
    println!("Exiting test...");
}

async fn send_camera(address: String, camera: PlacedCamera) -> tokio::io::Result<()> {
    use tokio::io::{stderr, AsyncWriteExt};

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

#[tokio::main]
async fn run() -> Result<(), String> {
    let location_service = LocationService::start(
        Some(Extrapolation::new::<LinearExtrapolation>(
            Duration::from_millis(500),
        )),
        camloc_common::hosts::constants::MAIN_PORT,
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

    fn write_to_stderr_binary(p: TimedPosition) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        use tokio::io::{stderr, AsyncWriteExt};
        Box::pin(async move {
            println!("{p}");

            let mut buf = 0i32.to_be_bytes().to_vec();
            buf.append(&mut [p.position.x.to_be_bytes(), p.position.y.to_be_bytes()].concat());

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

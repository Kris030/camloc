use camloc_server::{
    extrapolations::{LinearExtrapolation, Extrapolation},
    service::{LocationService, TimedPosition},
    calc::PlacedCamera,
};
use tokio::{time::sleep, sync::oneshot::{Sender, Receiver, self}};
use std::time::Duration;

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
        Some(
            Extrapolation::new::<LinearExtrapolation>(
                Duration::from_millis(500)
            )
        ), camloc_common::hosts::constants::MAIN_PORT
    ).await?;

    location_service.subscribe_connection(|address, camera| {
        let address = address.to_string();
        println!("New camera connected from {address}");
        tokio::spawn(async move { send_camera(address, camera).await });
    }).await;

    async fn write_to_stderr_binary(p: TimedPosition) {
        use tokio::io::{stderr, AsyncWriteExt};

        println!("{p}");

        let mut buf = 0i32.to_be_bytes().to_vec();
        buf.append(&mut [
            p.position.x.to_be_bytes(),
            p.position.y.to_be_bytes(),
        ].concat());

        stderr()
            .write_all(&buf[..]).await
            .expect("Couldn't write coords to stderr???");
    }

    static mut CHAN: (Option<Sender<()>>, Option<Receiver<()>>) = (None, None);
    unsafe {
        let (rx, tx) = oneshot::channel();
        CHAN = (Some(rx), Some(tx));
    }
    let mut rx = std::mem::replace(unsafe { &mut CHAN.1 }, None).unwrap();

    fn ctrlc_handler() {
        println!("ctrlc pressed");
        let Some(tx) = std::mem::replace(unsafe { &mut CHAN.0 }, None) else {
            return;
        };

        tx.send(()).unwrap();
    }
    let _ = ctrlc::set_handler(ctrlc_handler);

    if true {
        location_service.subscribe(|p| {
            tokio::spawn(write_to_stderr_binary(p));
        }).await;

        rx.await.map_err(|_| "Something failed in the channel")?;
        location_service.stop().await;
    } else {
        loop {
            if let Some(p) = location_service.get_position().await {
                write_to_stderr_binary(p).await;
            } else {
                println!("Couldn't get position");
            }

            let rec = rx.try_recv();
            use tokio::sync::oneshot::error::TryRecvError;
            match rec {
                Err(TryRecvError::Closed) => return Err("Channel closed???".to_string()),
                Err(TryRecvError::Empty) => sleep(Duration::from_millis(10)).await,
                Ok(()) => break,
            }
        }
    }

    Ok(())
}

use camloc::{
    extrapolations::{LinearExtrapolation, Extrapolation},
    service::{LocationService, Position, LocationServiceHandle},
    calc::PlacedCamera,
};
use std::time::Duration;
use tokio::time::sleep;
use ctrlc;

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

    se.write_f64(camera.pos.x).await?;
    se.write_f64(camera.pos.y).await?;
    se.write_f64(camera.rot).await?;

    se.write_f64(camera.info.fov).await?;

    Ok(())
}

#[tokio::main]
async fn run() -> Result<(), String> {
    let locations_service = LocationService::start(
        Some(
            Extrapolation::new::<LinearExtrapolation>(
                Duration::from_millis(500)
            )
        ), 1234
    ).await?;

    locations_service.subscribe_connection(|address, camera| {
        let address = address.to_string();
        println!("New camera connected from {address}");
        tokio::spawn(async move { send_camera(address, camera).await });
    }).await;

    async fn write_to_stderr_binary(p: Position) {
        use tokio::io::{stderr, AsyncWriteExt};

        println!("{p}");

        let mut buf = 0i32.to_be_bytes().to_vec();
        buf.append(&mut [
            p.coordinates.x.to_be_bytes(),
            p.coordinates.y.to_be_bytes(),
        ].concat());

        stderr()
            .write_all(&buf[..]).await
            .expect("Couldn't write coords to stderr???");
    }

    // let _ = ctrlc::set_handler(|| locations_service.stop_sync());

    if true {
        locations_service.subscribe(|p| {
            tokio::spawn(write_to_stderr_binary(p));
        }).await;
        sleep(Duration::from_secs(10000000)).await
    } else {
        let mut missing_positions = 0;
        while missing_positions < 100 {
            if let Some(p) = locations_service.get_position().await {
                write_to_stderr_binary(p).await;
                missing_positions = 0;
            } else {
                println!("Couldn't get position");
                missing_positions += 1;
            }
            sleep(Duration::from_millis(10)).await;
        }
    }

    Ok(())
}

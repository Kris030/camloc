use camloc::extrapolations::{LinearExtrapolation, Extrapolation};
use camloc::service::{LocationService, Position};
use camloc::calc::{Setup, CameraInfo};
use tokio::{time::sleep, fs};
use std::time::Duration;

fn main() {
    if let Err(e) = run() {
        println!("[ERROR]: {e}");
    } else {
        println!("No errors");
    }
    println!("Exiting test...");
}

async fn send_cameras(setup: &Setup, addresses: &[String]) -> tokio::io::Result<()> {
    #[allow(clippy::needless_range_loop)]
    for a in 0..setup.camera_count() {
        use tokio::io::{stderr, AsyncWriteExt};

        let mut se = stderr();
        se.write_i32(1).await?;
        se.write_u16(addresses[a].len() as u16).await?;
        se.write_all(addresses[a].as_bytes()).await?;

        let c = &setup.cameras()[a];
        se.write_f64(c.pos.x).await?;
        se.write_f64(c.pos.y).await?;
        se.write_f64(c.rot).await?;

        se.write_f64(c.info.fov).await?;

    }

    Ok(())
}

#[tokio::main]
async fn run() -> Result<(), String> {
    let picamera = CameraInfo::new(62.2f64.to_radians());
    let setup = Setup::new_square(3., vec![picamera; 2]);

    let addresses: Vec<String> = fs::read_to_string("address_lists/local.txt").await
        .map_err(|_| "Failed to read addresses".to_string())?
        .lines()
        .map(|l| l.to_string())
        .collect();

    send_cameras(&setup, &addresses).await
        .map_err(|_| "Couldn't write camera info".to_string())?;

    let extrapolation = Some(
        Extrapolation::new::<LinearExtrapolation>(
            Duration::from_millis(500)
        )
    );

    let locations_service = LocationService::start(
        setup,
        addresses,
        extrapolation,
    ).await?;

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

    if true {
        locations_service.subscribe(|p| {
            tokio::spawn(write_to_stderr_binary(p));
        }).await?;
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

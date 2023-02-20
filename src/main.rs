pub mod extrapolations;
pub mod service;
pub mod utils;
pub mod calc;

use extrapolations::{LinearExtrapolation, Extrapolation};
use service::{LocationService, Position};
use calc::{Setup, CameraInfo};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), String> {
    let picamera = CameraInfo::new((62.2, 48.8));
    let setup = Setup::new_square(3., [picamera; 2]);

    let addresses = [
        "localhost:12340",
        "localhost:12341",
    ];

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

        let buf = [
            p.coordinates.x.to_be_bytes(),
            p.coordinates.y.to_be_bytes(),
        ].concat();

        stderr()
            .write_all(&buf[..]).await
            .expect("Couldn't write coords to stderr???");
    }

    if false {
        locations_service.subscribe(|p| {
            tokio::spawn(async move {
                write_to_stderr_binary(p).await;
            });
        }).await?;
        sleep(Duration::from_secs(15)).await
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

    println!("Exiting test...");

    Ok(())
}

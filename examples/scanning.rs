use camloc::extrapolations::{LinearExtrapolation, Extrapolation};
use camloc::scanning::{AddressTemplate, TemplateMember::*};
use camloc::service::{LocationService, Position};
use camloc::calc::{Setup, CameraInfo};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), String> {
    let picamera = CameraInfo::new((62.2, 48.8));
    let setup = Setup::new_square(3., vec![picamera; 2]);

    let addresses = AddressTemplate::new(
        [Fixed(127), Fixed(0), Fixed(0), Fixed(1)],
        Templated(12340..12342)
    );

    let extrapolation = Some(
        Extrapolation::new::<LinearExtrapolation>(
            Duration::from_millis(500)
        )
    );

    let locations_service = LocationService::start_scanning(
        setup,
        addresses,
        extrapolation,
    ).await?;

    async fn write_to_stderr_binary(p: Position) {
        use tokio::io::{stderr, AsyncWriteExt};

        println!("{p}");

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
            tokio::spawn(write_to_stderr_binary(p));
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

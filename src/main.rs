pub mod extrapolations;
pub mod service;
pub mod calc;
pub mod utils;

use extrapolations::{LinearExtrapolation, Extrapolation};
use std::{time::Duration, thread::sleep};
use calc::{Setup, CameraInfo};

fn main() -> Result<(), String> {
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

    service::start(
        setup,
        addresses,
        extrapolation,
    )?;

    service::subscribe(|p| eprintln!("{p}"))?;

    for _ in 0..6 {
        if let Some(p) = service::get_position() {
            println!(" ?> {p}");
        } else {
            println!("Couldn't get position");
        }
        sleep(Duration::from_millis(100));
    }

    Ok(())
}

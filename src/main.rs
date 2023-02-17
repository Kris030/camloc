pub mod extrapolations;
pub mod service;
pub mod utils;
pub mod calc;

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

    let write_to_stderr_binary = |p: service::Position| {
        use std::io::{stderr, Write};

        println!("{p}");

        let buf = [
            p.coordinates.x.to_be_bytes(),
            p.coordinates.y.to_be_bytes(),
        ].concat();

        stderr()
            .lock()
            .write_all(&buf[..])
            .expect("Couldn't write coords to stderr???");
    };

    if false {
        service::subscribe(write_to_stderr_binary)?;
        sleep(Duration::from_secs(15));
    } else {
        let mut missing_positions = 0;
        while missing_positions < 100 {
            if let Some(p) = service::get_position() {
                write_to_stderr_binary(p);
                missing_positions = 0;
            } else {
                println!("Couldn't get position");
                missing_positions += 1;
            }
            sleep(Duration::from_millis(10));
        }
    }

    println!("Exiting test...");

    Ok(())
}

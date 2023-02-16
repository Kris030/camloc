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

    let _extrapolation = Some(
        Extrapolation::new::<LinearExtrapolation>(
            Duration::from_millis(500)
        )
    );

    service::start(
        setup,
        addresses,
        None, // extrapolation,
    )?;

    let write_to_stderr_binary = |p: service::Position| {
        use std::io::{stderr, Write};

        let buf = [
            p.coordinates.x.to_be_bytes(),
            p.coordinates.y.to_be_bytes(),
        ].concat();

        stderr()
            .lock()
            .write_all(&buf[..])
            .expect("Couldn't write coords to stderr???");
    };

    // service::subscribe(write_to_stderr_binary)?;

    for _ in 0..16 {
        if let Some(p) = service::get_position() {
            write_to_stderr_binary(p);
        } else {
            println!("Couldn't get position");
        }
        sleep(Duration::from_millis(30));
    }

    println!("Exiting test...");

    Ok(())
}

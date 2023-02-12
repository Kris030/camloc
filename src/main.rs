#![feature(array_try_map)]
#![feature(generic_arg_infer)]

pub mod service;
pub mod calc;

use std::{time::Duration, thread::sleep};
use calc::{Setup, CameraInfo};

fn main() -> Result<(), String> {
    let picamera = CameraInfo::new((62.2, 48.8));
    let setup = Setup::new_square(3., [picamera; 2]);

    let addresses = [
        "localhost:12340",
        "localhost:12341",
    ];

    let extrapolation = None;

    service::start(
        setup,
        addresses,
        extrapolation,
    )?;

    for _ in 0..6 {
        let p = service::get_position()
            .ok_or("Couldn't get position".to_owned())?;
        println!("({:.2}, {:.2})", p.0, p.1);
        sleep(Duration::from_millis(100));
    }

    Ok(())
}

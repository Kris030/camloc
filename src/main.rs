use std::{time::Duration, thread::sleep};

use crate::calc::{Setup, CameraInfo};

pub mod service;
pub mod calc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let picamera = CameraInfo::new((62.2, 48.2));

    let s = service::Service::start(
        Setup::new_square(3., [picamera; 2]),
        [
            "192.168.0.123".into(),
            "192.168.0.321".into(),
        ],
        None,
    )?;

    for _ in 0..10 {
        println!("{:?}", s.get_position());
        sleep(Duration::from_millis(500));
    }

    Ok(())
}

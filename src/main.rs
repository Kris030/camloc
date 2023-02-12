use std::{time::Duration, thread::sleep};

use crate::calc::{Setup, CameraInfo};

pub mod service;
pub mod calc;

fn main() -> Result<(), String> {
    let picamera = CameraInfo::new((62.2, 48.2));

    service::start(
        Setup::new_square(3., [picamera; 2]),
        [
            "192.168.0.123".into(),
            "192.168.0.321".into(),
        ],
        None,
    )?;

    for _ in 0..10 {
        println!("{:?}", service::get_position().ok_or("Couldn't get position".to_owned())?);
        sleep(Duration::from_millis(500));
    }

    Ok(())
}

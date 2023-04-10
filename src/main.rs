mod aruco;
mod util;

use aruco::Aruco;
use opencv::{highgui, prelude::*, videoio};

#[allow(unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;

    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut frame = Mat::default();
    let mut aruco = Aruco::new(2)?;

    loop {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }

        aruco.detect(&mut frame);

        highgui::imshow("videocap", &frame)?;
        highgui::wait_key(10)?;
    }

    // Ok(())
}

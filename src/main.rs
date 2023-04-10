mod aruco;
mod track;
mod util;

use aruco::Aruco;
use opencv::{highgui, prelude::*, videoio};
use track::Tracking;

#[allow(unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;

    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut frame = Mat::default();
    let mut draw = Mat::default();

    let mut aruco = Aruco::new(2)?;
    let mut tracker = Tracking::new()?;
    let mut has_object = false;

    loop {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }
        draw = frame.clone();

        // tracking logic
        if !has_object {
            if let Some(x) = aruco.detect(&mut frame, Some(&mut tracker.rect), Some(&mut draw))? {
                println!("{} | switching to tracking", x);
                has_object = true;
                tracker.init(&frame);
            }
        } else {
            if let Some(x) = tracker.track(&frame, Some(&mut draw))? {
                println!("{}", x);
            } else {
                println!("switching to detection");
                has_object = false;
            }
        }

        highgui::imshow("videocap", &draw)?;
        highgui::wait_key(10)?;
    }

    // Ok(())
}

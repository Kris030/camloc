mod aruco;
mod track;
mod util;

use aruco::Aruco;
use opencv::{highgui, prelude::*, videoio};
use std::io::Write;
use std::net::TcpListener;
use track::Tracking;

#[allow(unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;

    let listener = TcpListener::bind("0.0.0.0:1111")?;
    let port = listener.local_addr()?;

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
        println!("Waiting for connections on {}", port);
        let (mut tcp_stream, addr) = listener.accept()?;
        println!("Connection received from {:?}", addr);

        while highgui::wait_key(10)? != 113 {
            cam.read(&mut frame)?;
            if frame.size()?.width < 1 {
                continue;
            }
            draw = frame.clone();

            let mut final_x = f64::NAN;
            // tracking logic
            if !has_object {
                if let Some(x) =
                    aruco.detect(&mut frame, Some(&mut tracker.rect), Some(&mut draw))?
                {
                    final_x = x;
                    has_object = true;
                    tracker.init(&frame);
                    // println!("{} | switching to tracking", x.unwrap());
                }
            } else {
                if let Some(x) = tracker.track(&frame, Some(&mut draw))? {
                    final_x = x;
                    // println!("{}", x);
                } else {
                    has_object = false;
                    // println!("switching to detection");
                }
            }

            highgui::imshow("videocap", &draw)?;
            if tcp_stream.write_all(&final_x.to_be_bytes()).is_err() {
                break;
            }
        }
    }

    Ok(())
}

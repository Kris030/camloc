use opencv::{highgui, imgcodecs, prelude::*, videoio};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn take_samples() -> opencv::Result<()> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut frame = Mat::default();
    loop {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }

        highgui::imshow("videocap", &frame)?;

        match highgui::wait_key(10)? {
            // Q | esc
            113 | 27 => break,
            // space
            32 => {
                // take pic
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                imgcodecs::imwrite(
                    format!("img-{}.jpg", timestamp).as_str(),
                    &frame,
                    &opencv::core::Vector::<i32>::default(),
                )?;

                println!("image saved to `img-{}.jpg`", timestamp)
            }
            _ => (),
            // k => println!("{}", k),
        }
    }

    Ok(())
}

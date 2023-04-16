use opencv::{highgui, imgcodecs, prelude::*, videoio};

pub fn take_samples() -> opencv::Result<()> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut frame = Mat::default();
    let mut index = 0;
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
                imgcodecs::imwrite(
                    format!("img-{:0>3}.jpg", index).as_str(),
                    &frame,
                    &opencv::core::Vector::<i32>::default(),
                )?;

                println!("image saved to `img-{:0>3}.jpg`", index);
                index += 1;
            }
            _ => (),
            // k => println!("{}", k),
        }
    }

    Ok(())
}

use opencv::{highgui, imgcodecs, prelude::*, videoio};

use crate::calibrate::load_camera_params;

pub fn take_samples(filename: Option<String>) -> opencv::Result<()> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut camera_matrix = Mat::default();
    let mut optimal_matrix = Mat::default();
    let mut dist_coeffs = Mat::default();
    if let Some(f) = filename.as_ref() {
        load_camera_params(
            f.as_str(),
            &mut camera_matrix,
            &mut dist_coeffs,
            &mut optimal_matrix,
        )?;
    }

    let mut frame = Mat::default();
    let mut draw = Mat::default();
    let mut index = 0;
    loop {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }
        frame.copy_to(&mut draw)?;

        if filename.is_some() {
            opencv::calib3d::undistort(
                &frame,
                &mut draw,
                &camera_matrix,
                &dist_coeffs,
                &optimal_matrix,
            )?;
        }

        highgui::imshow("videocap", &draw)?;
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

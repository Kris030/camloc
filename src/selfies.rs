use camloc_common::calibration::{draw_charuco_board, find_board};
use opencv::{highgui, imgcodecs, objdetect::CharucoBoard, prelude::*, videoio};

use crate::calibrate::load_camera_params;

pub fn take_samples(board: &CharucoBoard, filename: Option<String>) -> opencv::Result<()> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let camera_params = if let Some(f) = filename {
        Some(load_camera_params(&f)?)
    } else {
        None
    };

    let mut frame = Mat::default();
    let mut draw = Mat::default();
    let mut index = 0;

    loop {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }

        if let Some(p) = &camera_params {
            opencv::calib3d::undistort(
                &frame,
                &mut draw,
                &p.camera_matrix,
                &p.dist_coeffs,
                &p.optimal_matrix,
            )?;
        } else {
            frame.copy_to(&mut draw)?;
        }

        if let Some(fb) = find_board(&draw, board, true)? {
            draw_charuco_board(&mut draw, &fb)?;
        }

        highgui::imshow("videocap", &draw)?;
        match highgui::wait_key(10)? {
            // Q | esc
            113 | 27 => break,
            // space
            32 => {
                imgcodecs::imwrite(
                    format!("img-{index:0>3}.jpg").as_str(),
                    &frame,
                    &opencv::core::Vector::<i32>::default(),
                )?;

                println!("image saved to `img-{index:0>3}.jpg`");
                index += 1;
            }
            _ => (),
        }
    }

    Ok(())
}

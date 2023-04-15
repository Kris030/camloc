use opencv::{aruco::calibrate_camera_charuco, core::TermCriteria, types::PtrOfCharucoBoard};
#[allow(unused)]
use opencv::{core, objdetect::CharucoBoard, prelude::*, types};

#[allow(unused)]
pub fn calibrate(board: &CharucoBoard) -> opencv::Result<()> {
    let image_size = core::Size {
        width: 640,
        height: 480,
    };

    let charuco_corners = types::VectorOfVectorOfPoint2f::new();
    let charuco_ids = types::VectorOfVec2i::new();

    let mut camera_matrix = Mat::default();
    let mut dist_coeffs = Mat::default();
    let mut rvecs = core::no_array();
    let mut tvecs = core::no_array();
    let flags = 0;

    let board = PtrOfCharucoBoard::new(board.clone());

    let est = calibrate_camera_charuco(
        &charuco_corners,
        &charuco_ids,
        &board,
        image_size,
        &mut camera_matrix,
        &mut dist_coeffs,
        &mut rvecs,
        &mut tvecs,
        flags,
        TermCriteria::default()?,
    )?;

    Ok(())
}

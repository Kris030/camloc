use opencv::{
    objdetect::{self, CharucoDetector, CharucoParameters, CharucoBoard},
    core::{self, FileStorage, TermCriteria},
    calib3d::get_optimal_new_camera_matrix, 
    aruco::calibrate_camera_charuco,
    prelude::*,
    highgui,
    videoio,
    types,
};

pub fn detect_all_boards(
    board: &CharucoBoard,
    all_charuco_corners: &mut types::VectorOfVectorOfPoint2f,
    all_charuco_ids: &mut types::VectorOfVectorOfi32,
    delay: i32,
) -> opencv::Result<()> {
    let mut cap = videoio::VideoCapture::from_file("img-%3d.jpg", videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cap)? {
        panic!("no sample images found!");
    }

    let mut _rejected = types::VectorOfVectorOfPoint2f::new();
    let marker_detector = objdetect::ArucoDetector::new(
        &objdetect::get_predefined_dictionary(objdetect::PredefinedDictionaryType::DICT_4X4_50)?,
        &objdetect::DetectorParameters::default()?,
        objdetect::RefineParameters {
            min_rep_distance: 0.5,
            error_correction_rate: 1.0,
            check_all_orders: true,
        },
    )?;

    let charuco_detector = CharucoDetector::new(
        board,
        &CharucoParameters::default()?,
        &objdetect::DetectorParameters::default()?,
        objdetect::RefineParameters {
            min_rep_distance: 0.5,
            error_correction_rate: 1.0,
            check_all_orders: true,
        },
    )?;

    let mut marker_corners = types::VectorOfVectorOfPoint2f::new();
    let mut marker_ids = types::VectorOfi32::new();
    let mut charuco_corners = types::VectorOfPoint2f::new();
    let mut charuco_ids = types::VectorOfi32::new();

    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut frame = Mat::default();
    let mut draw = Mat::default();

    loop {
        cap.read(&mut frame)?;
        if frame.size()?.width < 1 {
            break;
        }
        frame.copy_to(&mut draw)?;

        // detect
        marker_detector.detect_markers(
            &frame,
            &mut marker_corners,
            &mut marker_ids,
            &mut _rejected,
        )?;

        // requires at least one detectable marker
        if marker_ids.is_empty() {
            continue;
        }

        // moved from interpolate_corners_charuco
        charuco_detector.detect_board(
            &frame,
            &mut charuco_corners,
            &mut charuco_ids,
            &mut marker_corners,
            &mut marker_ids,
        )?;

        if charuco_ids.is_empty() {
            continue;
        }

        // push
        all_charuco_corners.push(charuco_corners.clone());
        all_charuco_ids.push(charuco_ids.clone());

        // draw
        objdetect::draw_detected_markers(
            &mut draw,
            &marker_corners,
            &marker_ids,
            core::Scalar::new(0.0, 255.0, 0.0, 1.0),
        )?;
        objdetect::draw_detected_corners_charuco(
            &mut draw,
            &charuco_corners,
            &charuco_ids,
            core::Scalar::new(0.0, 0.0, 255.0, 1.0),
        )?;

        highgui::imshow("videocap", &draw)?;
        highgui::wait_key(delay)?;
    }

    Ok(())
}

pub fn calibrate(board: &CharucoBoard, delay: i32, filename: &str) -> opencv::Result<()> {
    let mut charuco_corners = types::VectorOfVectorOfPoint2f::new();
    let mut charuco_ids = types::VectorOfVectorOfi32::new();

    detect_all_boards(board, &mut charuco_corners, &mut charuco_ids, delay)?;

    let image_size = core::Size {
        width: 640,
        height: 480,
    };
    let mut camera_matrix = Mat::default();
    let mut dist_coeffs = Mat::default();
    let mut rvecs = types::VectorOfMat::new();
    let mut tvecs = types::VectorOfMat::new();
    let flags = 0;

    let board = types::PtrOfCharucoBoard::new(board.clone());
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

    println!("calibration finished\nestimated calibration error: {est:.3}");

    let optimal_matrix = get_optimal_new_camera_matrix(
        &camera_matrix,
        &dist_coeffs,
        image_size,
        0.2,
        image_size,
        None,
        false,
    )?;

    save_camera_params(filename, &camera_matrix, &dist_coeffs, &optimal_matrix)?;

    Ok(())
}

pub fn save_camera_params(
    filename: &str,
    camera_matrix: &Mat,
    dist_coeffs: &Mat,
    optimal_matrix: &Mat,
) -> opencv::Result<()> {
    let mut fs = FileStorage::new(filename, core::FileStorage_WRITE, "")?;

    fs.write_mat("camera_matrix", camera_matrix)?;
    fs.write_mat("dist_coeffs", dist_coeffs)?;
    fs.write_mat("optimal_matrix", optimal_matrix)?;

    fs.release()?;
    Ok(())
}

pub fn load_camera_params(
    filename: &str,
    camera_matrix: &mut Mat,
    dist_coeffs: &mut Mat,
    optimal_matrix: &mut Mat,
) -> opencv::Result<()> {
    let mut fs = FileStorage::new(filename, core::FileStorage_READ, "")?;

    fs.get("camera_matrix")?.mat()?.copy_to(camera_matrix)?;
    fs.get("dist_coeffs")?.mat()?.copy_to(dist_coeffs)?;
    fs.get("optimal_matrix")?.mat()?.copy_to(optimal_matrix)?;

    fs.release()?;
    Ok(())
}

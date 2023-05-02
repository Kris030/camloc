use opencv::{
    objdetect::{self, CharucoBoard, CharucoDetector, CharucoParameters},
    prelude::*,
    highgui,
    types,
    core, aruco::calibrate_camera_charuco, calib3d::get_optimal_new_camera_matrix,
};

pub fn generate_board(width: u8, height: u8) -> opencv::Result<CharucoBoard> {
    CharucoBoard::new(
        core::Size::new(width as i32, height as i32),
        0.04,
        0.02,
        &objdetect::get_predefined_dictionary(objdetect::PredefinedDictionaryType::DICT_4X4_50)?,
        &core::no_array(),
    )
}

pub fn find_board(image: &Mat, board: &CharucoBoard, include_markers: bool) -> opencv::Result<Option<FoundBoard>> {
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
    let mut corners = types::VectorOfPoint2f::new();
    let mut ids = types::VectorOfi32::new();

    // detect
    marker_detector.detect_markers(
        &image,
        &mut marker_corners,
        &mut marker_ids,
        &mut core::no_array(),
    )?;

    // requires at least one detectable marker
    if marker_ids.is_empty() {
        return Ok(None);
    }

    // moved from interpolate_corners_charuco
    charuco_detector.detect_board(
        &image,
        &mut corners,
        &mut ids,
        &mut marker_corners,
        &mut marker_ids,
    )?;

    if ids.is_empty() {
        return Ok(None);
    }

    let markers = if include_markers {
        Some(FoundMarkers { corners: marker_corners, ids: marker_ids })
    } else {
        None
    };

    Ok(Some(FoundBoard { corners, ids, markers }))
}

pub fn display_image(image: &Mat, title: &str, destroy: bool) -> opencv::Result<()> {
    highgui::imshow(title, image)?;

    while !matches!(
        highgui::wait_key(0),
        Err(_) | Ok(113)
    ) {}

    if destroy {
        highgui::destroy_window(title)?;
    }
    Ok(())
}

pub fn draw_board(image: &mut Mat, board: &FoundBoard) -> opencv::Result<()> {
    objdetect::draw_detected_corners_charuco(
        image,
        &board.corners,
        &board.ids,
        core::Scalar::new(0.0, 0.0, 255.0, 1.0),
    )?;
    Ok(())
}
pub fn draw_charuco_board(image: &mut Mat, board: &FoundBoard) -> opencv::Result<()> {
    draw_board(image, board)?;
    if let Some(markers) = &board.markers {
        objdetect::draw_detected_markers(
            image,
            &markers.corners,
            &markers.ids,
            core::Scalar::new(0.0, 255.0, 0.0, 1.0),
        )?;
    }
    Ok(())
}

pub fn calibrate(board: &CharucoBoard, images: &[Mat]) -> opencv::Result<FullCameraInfo> {
    let (mut charuco_corners, mut charuco_ids) = (types::VectorOfVectorOfPoint2f::new(), types::VectorOfVectorOfi32::new());
    for img in images {
        if let Some(fb) = find_board(img, board, false)? {
            charuco_corners.push(fb.corners);
            charuco_ids.push(fb.ids);
        }
    }

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
        core::TermCriteria::default()?,
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

    let k = camera_matrix.to_vec_2d::<f64>()?
        .into_iter()
        .flatten()
        .collect::<Vec<f64>>();
    let k: [f64; 9] = k.as_slice().try_into().unwrap();
    let k = core::Matx::from_array(k);
    let cam = opencv::viz::Camera::new_2(k, image_size)?;

    let [horizontal, vertical] = cam.get_fov()?.0;

    Ok(FullCameraInfo {
        params: CameraParams {
            camera_matrix,
            dist_coeffs,
            optimal_matrix,
        },
        fov: CameraFOV { horizontal, vertical }
    })
}

pub struct CameraFOV {
    pub horizontal: f64,
    pub vertical: f64,  
}

pub struct CameraParams {
    pub optimal_matrix: Mat,
    pub camera_matrix: Mat,
    pub dist_coeffs: Mat,
}

pub struct FullCameraInfo {
    pub params: CameraParams,
    pub fov: CameraFOV,
}

pub struct FoundBoard {
    corners: types::VectorOfPoint2f,
    ids: types::VectorOfi32,
    markers: Option<FoundMarkers>,
}

pub struct FoundMarkers {
    corners: types::VectorOfVectorOfPoint2f,
    ids: types::VectorOfi32,
}

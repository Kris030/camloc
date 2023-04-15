use opencv::core::FileStorage;
use opencv::objdetect::{self, CharucoDetector, CharucoParameters};
use opencv::{aruco::calibrate_camera_charuco, core::TermCriteria, types::PtrOfCharucoBoard};
use opencv::{core, highgui, objdetect::CharucoBoard, prelude::*, types, videoio};

fn detect_boards(
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
    let mut draw: Mat;

    loop {
        cap.read(&mut frame)?;
        if frame.size()?.width < 1 {
            break;
        }
        draw = frame.clone();

        // detect
        marker_detector.detect_markers(
            &frame,
            &mut marker_corners,
            &mut marker_ids,
            &mut _rejected,
        )?;

        // requires at least one detectable marker
        if marker_ids.len() == 0 {
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

        if charuco_ids.len() == 0 {
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

pub fn calibrate(board: &CharucoBoard, delay: i32) -> opencv::Result<()> {
    let mut charuco_corners = types::VectorOfVectorOfPoint2f::new();
    let mut charuco_ids = types::VectorOfVectorOfi32::new();

    detect_boards(board, &mut charuco_corners, &mut charuco_ids, delay)?;

    let image_size = core::Size {
        width: 640,
        height: 480,
    };
    let mut camera_matrix = Mat::default();
    let mut dist_coeffs = Mat::default();
    let mut rvecs = types::VectorOfMat::new();
    let mut tvecs = types::VectorOfMat::new();
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

    // println!("{:?}", camera_matrix);
    // println!("{:?}", dist_coeffs);
    // println!("{:?}", rvecs);
    // println!("{:?}", tvecs);

    println!(
        "calibration finished\nestimated calibration error: {:.3}",
        est
    );

    Ok(())
}

#[allow(unused)]
fn save_camera_params(
    camera_matrix: &Mat,
    dist_coeffs: &Mat,
    rvecs: &types::VectorOfMat,
    tvecs: &types::VectorOfMat,
) -> opencv::Result<()> {
    let mut fs = FileStorage::new("test.xml", core::FileStorage_WRITE, "")?;

    fs.write_mat("camera_matrix", camera_matrix)?;
    fs.write_mat("dist_coeffs", dist_coeffs)?;
    // fs.write_mat("rvecs", rvecs)?;
    // fs.write_mat("tvecs", tvecs)?;

    fs.release();
    Ok(())
}

// static void saveCameraParams( const string& filename,
//     Size imageSize, Size boardSize,
//     float squareSize, float aspectRatio, int flags,
//     const Mat& cameraMatrix, const Mat& distCoeffs,
//     const vector<Mat>& rvecs, const vector<Mat>& tvecs,
//     const vector<float>& reprojErrs,
//     const vector<vector<Point2f> >& imagePoints,
//     double totalAvgErr )
// {
// FileStorage fs( filename, FileStorage::WRITE );

// time_t tt;
// time( &tt );
// struct tm *t2 = localtime( &tt );
// char buf[1024];
// strftime( buf, sizeof(buf)-1, "%c", t2 );

// fs << "calibration_time" << buf;

// if( !rvecs.empty() || !reprojErrs.empty() )
// fs << "nframes" << (int)std::max(rvecs.size(), reprojErrs.size());
// fs << "image_width" << imageSize.width;
// fs << "image_height" << imageSize.height;
// fs << "board_width" << boardSize.width;
// fs << "board_height" << boardSize.height;
// fs << "square_size" << squareSize;

// if( flags & CV_CALIB_FIX_ASPECT_RATIO )
// fs << "aspectRatio" << aspectRatio;

// if( flags != 0 )
// {
// sprintf( buf, "flags: %s%s%s%s",
// flags & CV_CALIB_USE_INTRINSIC_GUESS ? "+use_intrinsic_guess" : "",
// flags & CV_CALIB_FIX_ASPECT_RATIO ? "+fix_aspectRatio" : "",
// flags & CV_CALIB_FIX_PRINCIPAL_POINT ? "+fix_principal_point" : "",
// flags & CV_CALIB_ZERO_TANGENT_DIST ? "+zero_tangent_dist" : "" );
// cvWriteComment( *fs, buf, 0 );
// }

// fs << "flags" << flags;

// fs << "camera_matrix" << cameraMatrix;
// fs << "distortion_coefficients" << distCoeffs;

// fs << "avg_reprojection_error" << totalAvgErr;
// if( !reprojErrs.empty() )
// fs << "per_view_reprojection_errors" << Mat(reprojErrs);

// if( !rvecs.empty() && !tvecs.empty() )
// {
// CV_Assert(rvecs[0].type() == tvecs[0].type());
// Mat bigmat((int)rvecs.size(), 6, rvecs[0].type());
// for( int i = 0; i < (int)rvecs.size(); i++ )
// {
// Mat r = bigmat(Range(i, i+1), Range(0,3));
// Mat t = bigmat(Range(i, i+1), Range(3,6));

// CV_Assert(rvecs[i].rows == 3 && rvecs[i].cols == 1);
// CV_Assert(tvecs[i].rows == 3 && tvecs[i].cols == 1);
// //*.t() is MatExpr (not Mat) so we can use assignment operator
// r = rvecs[i].t();
// t = tvecs[i].t();
// }
// cvWriteComment( *fs, "a set of 6-tuples (rotation vector + translation vector) for each view", 0 );
// fs << "extrinsic_parameters" << bigmat;
// }

// if( !imagePoints.empty() )
// {
// Mat imagePtMat((int)imagePoints.size(), (int)imagePoints[0].size(), CV_32FC2);
// for( int i = 0; i < (int)imagePoints.size(); i++ )
// {
// Mat r = imagePtMat.row(i).reshape(2, imagePtMat.cols);
// Mat imgpti(imagePoints[i]);
// imgpti.copyTo(r);
// }
// fs << "image_points" << imagePtMat;
// }
// }

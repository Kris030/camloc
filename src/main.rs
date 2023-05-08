use camloc_common::calibration::{draw_charuco_board, find_board, generate_board, CameraParams};
use clap::{Parser, Subcommand};
use opencv::{
    core::{self, TermCriteria},
    highgui, imgcodecs,
    objdetect::{self, CharucoBoard, CharucoDetector, CharucoParameters},
    prelude::*,
    types,
    videoio::VideoCapture,
};

#[derive(Parser)]
struct Args {
    /// Number of rows in the generated image (should be odd)
    #[arg(short, long, default_value_t = 5)]
    rows: u8,

    /// Number of columns in the generated image (should be odd)
    #[arg(short, long, default_value_t = 7)]
    cols: u8,

    /// The action to take
    #[command(subcommand)]
    command: CLICommand,
}

#[derive(Subcommand)]
enum CLICommand {
    /// Generates Charuco board image
    Generate {
        /// File name to save generated board to
        #[arg(long, default_value = "board.png")]
        file: String,

        /// Margin around the generated image (px)
        #[arg(long, default_value_t = 20)]
        margin: u16,

        /// Resolution of a single tile (px)
        #[arg(long, default_value_t = 200)]
        resolution: u16,
    },

    /// Take photos for calibration using the generated board
    Selfie {
        /// An existing config to use for undistortion
        #[arg(short, long)]
        existing_config: Option<String>,

        /// Camera index to open
        #[arg(short, long, default_value_t = 0)]
        camera_index: i32,

        /// Filename template
        #[arg(short, long, default_value = "img-%i.jpg")]
        template: String,
    },

    /// Acquire camera parameters
    Calibrate {
        /// Camera index to open
        #[arg(long, default_value_t = 0)]
        camera_index: i32,

        /// Camera or files
        #[command(subcommand)]
        command: CalibrateCommand,

        /// File to save to
        #[arg(short, long, default_value = ".calib")]
        savefile: String,

        /// The width to calibrate for
        #[arg(short = 'w', long)]
        image_width: u16,

        /// The heigth to calibrate for
        #[arg(short = 'h', long)]
        image_height: u16,
    },
}

#[derive(Subcommand)]
enum CalibrateCommand {
    /// Take images from camera
    FromCamera,
    /// Load images from files
    FromFiles { files: Vec<String> },
}

fn main() -> opencv::Result<()> {
    let args = Args::parse();
    let board = generate_board(args.cols, args.rows)?;

    match args.command {
        CLICommand::Generate {
            file,
            margin,
            resolution,
        } => {
            let mut img = Mat::default();
            let size = board.get_chessboard_size()?;
            board.generate_image(
                core::Size {
                    width: size.width * resolution as i32,
                    height: size.height * resolution as i32,
                },
                &mut img,
                margin as i32,
                1,
            )?;
            imgcodecs::imwrite(&file, &img, &core::Vector::new())?;
            println!("board successfully exported to `{file}`");
        }

        CLICommand::Selfie {
            existing_config,
            camera_index,
            template,
        } => {
            let v = core::Vector::new();
            for (i, s) in take_samples(&board, existing_config, camera_index)?
                .into_iter()
                .enumerate()
            {
                imgcodecs::imwrite(&template.replace("%i", &format!("{i:0>3}")), &s, &v)?;
            }
        }

        CLICommand::Calibrate {
            command,
            savefile,
            camera_index,
            image_width,
            image_height,
        } => {
            let images = match command {
                CalibrateCommand::FromCamera => take_samples(&board, None, camera_index)?,
                CalibrateCommand::FromFiles { files } => files
                    .into_iter()
                    .map(|f| imgcodecs::imread(&f, imgcodecs::IMREAD_COLOR))
                    .collect::<opencv::Result<_>>()?,
            };

            if images.is_empty() {
                println!("No images");
                return Ok(());
            }

            calibrate(
                &board,
                &savefile,
                &images,
                core::Size::new(image_width as i32, image_height as i32),
            )?
        }
    };

    Ok(())
}

pub fn take_samples(
    board: &CharucoBoard,
    filename: Option<String>,
    camera_index: i32,
) -> opencv::Result<Vec<Mat>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = VideoCapture::new(camera_index, opencv::videoio::CAP_ANY)?;
    if !cam.is_opened()? {
        panic!("camera index not found!");
    }

    let camera_params = if let Some(f) = filename {
        Some(CameraParams::load(&f)?)
    } else {
        None
    };

    let mut frame = Mat::default();
    let mut draw = Mat::default();

    let mut ret = vec![];
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
            32 => ret.push(frame.clone()),
            _ => (),
        }
    }

    Ok(ret)
}

pub fn detect_all_boards(
    board: &CharucoBoard,
    all_charuco_corners: &mut types::VectorOfVectorOfPoint2f,
    all_charuco_ids: &mut types::VectorOfVectorOfi32,
    images: &[Mat],
) -> opencv::Result<()> {
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

    for frame in images {
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
    }

    Ok(())
}

pub fn calibrate(
    board: &CharucoBoard,
    filename: &str,
    images: &[Mat],
    image_size: core::Size,
) -> opencv::Result<()> {
    let mut charuco_corners = types::VectorOfVectorOfPoint2f::new();
    let mut charuco_ids = types::VectorOfVectorOfi32::new();

    detect_all_boards(board, &mut charuco_corners, &mut charuco_ids, images)?;

    let mut camera_matrix = Mat::default();
    let mut dist_coeffs = Mat::default();
    let mut rvecs = types::VectorOfMat::new();
    let mut tvecs = types::VectorOfMat::new();
    let flags = 0;

    let board = types::PtrOfCharucoBoard::new(board.clone());
    let est = opencv::aruco::calibrate_camera_charuco(
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

    let optimal_matrix = opencv::calib3d::get_optimal_new_camera_matrix(
        &camera_matrix,
        &dist_coeffs,
        image_size,
        0.2,
        image_size,
        None,
        false,
    )?;

    CameraParams {
        camera_matrix,
        dist_coeffs,
        optimal_matrix,
    }
    .save(filename)?;

    Ok(())
}

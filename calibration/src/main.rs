use camloc_common::cv::{calibrate, draw_charuco_board, find_board, generate_board, CameraParams};
use clap::{Parser, Subcommand};
use opencv::{
    self, core, highgui, imgcodecs, objdetect::CharucoBoard, prelude::*, videoio::VideoCapture,
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

            let fci = calibrate(
                &board,
                &images,
                core::Size::new(image_width as i32, image_height as i32),
            )?;

            std::fs::write(savefile, fci.to_be_bytes()).unwrap();
        }
    };

    Ok(())
}

fn take_samples(
    board: &CharucoBoard,
    filename: Option<String>,
    camera_index: i32,
) -> opencv::Result<Vec<Mat>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = VideoCapture::new(camera_index, opencv::videoio::CAP_ANY)?;

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

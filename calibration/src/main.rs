mod calibrate;
mod generate;
mod selfies;

use crate::generate::export_board;

use clap::Parser;
use generate::generate_board;
use opencv::{highgui, prelude::*, videoio};
use selfies::take_samples;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Generates Charuco board image
    #[arg(long)]
    generate: bool,

    /// Take photos for calibration using the generated board
    #[arg(long)]
    take_samples: bool,

    /// Use calibration photos to export camera properties
    #[arg(long)]
    calibrate: bool,

    /// Number of rows in the image (should be odd)
    #[arg(long, default_value_t = 5)]
    rows: i32,

    /// Number of columns in the image (should be odd)
    #[arg(long, default_value_t = 7)]
    cols: i32,

    /// File name to save board to
    #[arg(long, default_value_t = String::from("target.jpg"))]
    name: String,

    /// Margin around the image (px)
    #[arg(long, default_value_t = 20)]
    margin: i32,

    /// Resolution of a single tile (px)
    #[arg(long, default_value_t = 200)]
    tile_res: i32,

    /// Camera index to open
    #[arg(long, default_value_t = 0)]
    camera_index: i32,
}

#[allow(unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let board = generate_board(args.cols, args.rows)?;

    if args.generate {
        export_board(&board, args.margin, args.tile_res, &args.name)?;
        println!("board successfully exported to `{}`", args.name);
        return Ok(());
    }

    if args.take_samples {
        take_samples()?;
        return Ok(());
    }

    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;

    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }

    let mut frame = Mat::default();
    let mut draw = Mat::default();

    while highgui::wait_key(10)? != 113 {
        cam.read(&mut frame)?;
        if frame.size()?.width < 1 {
            continue;
        }

        draw = frame.clone();

        highgui::imshow("videocap", &draw)?;
    }

    Ok(())
}

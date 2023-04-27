mod calibrate;
mod generate;
mod selfies;

use crate::generate::export_board;

use camloc_common::calibration::generate_board;
use selfies::take_samples;
use calibrate::calibrate;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Generates Charuco board image
    #[arg(long)]
    generate: bool,

    /// Take photos for calibration using the generated board
    #[arg(long)]
    selfie: bool,

    /// Calibrate using sample photos and export calibration as <filename>
    #[arg(long)]
    calibrate: Option<String>,

    /// When using selfie, undistort image using <filename>
    #[arg(long)]
    undistort: Option<String>,

    /// Number of rows in the generated image (should be odd)
    #[arg(long, default_value_t = 5)]
    rows: i32,

    /// Number of columns in the generated image (should be odd)
    #[arg(long, default_value_t = 7)]
    cols: i32,

    /// File name to save generated board to
    #[arg(long, default_value_t = String::from("target.jpg"))]
    name: String,

    /// Margin around the generated image (px)
    #[arg(long, default_value_t = 20)]
    margin: i32,

    /// Resolution of a single tile (px)
    #[arg(long, default_value_t = 200)]
    tile_res: i32,

    /// How long should calibrated images be shown
    #[arg(long, default_value_t = 100)]
    delay: i32,

    /// Camera index to open
    #[arg(long, default_value_t = 0)]
    camera_index: i32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let board = generate_board(args.cols, args.rows)?;

    if args.generate {
        export_board(&board, args.margin, args.tile_res, &args.name)?;
    } else if args.selfie {
        take_samples(args.undistort)?;
    } else if let Some(save) = args.calibrate {
        calibrate(&board, args.delay, save.as_str())?;
    }

    Ok(())
}

use opencv::{highgui, prelude::*, videoio, Result, imgcodecs::{self, imread}, imgproc};

static WNAME: &'static str = "OPENCV Testo";

// ---- OPENCV ----

pub fn kepu() -> Result<()> {
    let img = imread("/home/moi/dev/robo/camloc/test/1.jpg", imgcodecs::IMREAD_COLOR)?;
    if img.empty() {
		panic!("Error opening image");
	}
    
	highgui::named_window(WNAME, highgui::WINDOW_AUTOSIZE)?;
	
	let mut p = opencv::core::Point::from((100, 100));

	let (w, h) = (img.cols(), img.rows());

    loop {
		let mut img = img.clone();
		
		imgproc::line(&mut img,
			opencv::core::Point::from((w / 2, h / 2)), p,
			opencv::core::Scalar::from((0., 0., 255.)), 2,
			opencv::imgproc::LINE_8, 0
		)?;
		imgproc::circle(&mut img,
			p, 4, opencv::core::Scalar::from((0., 0., 255.)),
			-1, opencv::imgproc::LINE_8, 0
		)?;
		highgui::imshow(WNAME, &img)?;

		// ---
        let Some(k) = char::from_u32(highgui::wait_key(50)? as u32) else {
            continue;
        };
        match k {
            'q' => break,
			
			'w' => p.y -= 1,
			's' => p.y += 1,

			'd' => p.x += 1,
			'a' => p.x -= 1,
            
			_ => (),
        }
    }
    Ok(())
}

pub fn videjo() -> Result<()> {
	highgui::named_window(WNAME, highgui::WINDOW_AUTOSIZE)?;

	let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?; // 0 is the default camera
	if !videoio::VideoCapture::is_opened(&cam)? {
		panic!("Unable to open default camera!");
	}

	loop {
		let mut frame = Mat::default();
		cam.read(&mut frame)?;
		if frame.size()?.width > 0 {
			highgui::imshow(WNAME, &frame)?;
		}

		let key = highgui::wait_key(10)?;
		if key > 0 && key != 255 {
			break;
		}
	}

	Ok(())
}

// ---- POS ----

extern crate uom;

use uom::si::length::{meter, millimeter, micrometer, inch};
use uom::si::angle::degree;
use uom::si::ratio::ratio;

use uom::si::f64::{Length, Angle, Ratio};
use uom::ConstZero;

/// Physical characteristics of the "arena"
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct ArenaInfo {
    /// Length of the arena walls
    pub square_size: Length,
}

/// Physical characteristics of a camera
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CameraInfo {

    /// Dimensions of the physical camera module
    pub camera_module_size: (Length, Length, Length),

    /// Horizontal, vertical Field Of View
    pub fov: (Angle, Angle),

    /// Image resolution
    pub camera_resolution: (u32, u32),

    /// Width and height of the image sensor
    pub sensor_image_area: (Length, Length),
    /// Length of the image sensor diagonal
    pub sensor_diagonal: Length,

    /// Focal length
    pub focal_length: Length,

    /// Pixel size
    pub pixel_size: (Length, Length),

    /// Optical size
    pub optical_size: Length,
}

/// The "playfield" setup
#[non_exhaustive]
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Setup {
    pub arena: ArenaInfo,
    pub cameras: [CameraInfo; 2],

    /// Distance of cameras from the center (calculated)
    pub camera_positions: [(Length, Length); 2],
}

impl Setup {
    pub fn new(arena: ArenaInfo, cameras: [CameraInfo; 2]) -> Self {
		let cd = |c: CameraInfo|
			0.5 * arena.square_size * (
            Ratio::new::<ratio>(1.) +
            1. / (0.5 * c.fov.0).tan()
        );

        Self {
			arena, cameras,
			camera_positions: [
				(-cd(cameras[0]), Length::ZERO),
				(Length::ZERO, cd(cameras[1])),
			],
		}
    }

    pub fn calculate_position(&self, px1: f64, px2: f64) -> (Length, Length) {
		let c0p: &(Length, Length) = &self.camera_positions[0];
		let c1p: &(Length, Length) = &self.camera_positions[1];
		
		let fov0: Angle = self.cameras[0].fov.0;
		let fov1: Angle = self.cameras[1].fov.0;
		
		let alpha: Angle = fov0 * (0.5 - px1);
		let alphatan: Ratio = alpha.tan();
		
		let beta: Angle = Angle::new::<degree>(90.) + fov1 * (px2 - 1.);
		let betan: Ratio = beta.tan();

		let x: Length = (alphatan * c0p.0 + c0p.1
						-betan * c1p.0 - c1p.1)
							/ (alphatan - betan);

        (x, Length::ZERO)
    }
}

fn main() {
	let picamera = CameraInfo {
        camera_module_size: (
            Length::new::<millimeter>(25.0),
            Length::new::<millimeter>(24.0),
            Length::new::<millimeter>(9.00),
        ),

        fov: ( 
            Angle::new::<degree>(62.2),
            Angle::new::<degree>(48.8),
        ),

        camera_resolution: (3280, 2464),

        sensor_image_area: (
            Length::new::<millimeter>(3.68),
            Length::new::<millimeter>(2.76),
        ),
        sensor_diagonal: Length::new::<millimeter>(4.6),

        focal_length: Length::new::<millimeter>(3.04),

        pixel_size: (
            Length::new::<micrometer>(1.12),
            Length::new::<micrometer>(1.12),
        ),

        optical_size: Length::new::<inch>(0.25),
    };
    let setup = Setup::new(
        ArenaInfo { square_size: Length::new::<meter>(3.0), },
        [picamera, picamera]
	);

    println!(
        "Camera positions: {:?}",
        setup.camera_positions,
    );

    println!(
        "Robot position: {:?}",
        setup.calculate_position(0.48.into(), 0.59.into()),
    );

}


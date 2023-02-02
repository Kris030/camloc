use opencv::{highgui, prelude::*, videoio, Result, imgcodecs::{self, imread}, imgproc};

static WNAME: &'static str = "OPENCV Testo";

fn main() -> Result<()> {
    kepu()
}

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
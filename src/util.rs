use opencv::{core, imgproc, prelude::*, types};

pub fn draw_rect(
    frame: &mut Mat,
    bounding: &types::VectorOfPoint2f,
) -> Result<(), Box<dyn std::error::Error>> {
    if bounding.len() < 4 {
        return Ok(());
    }

    let mut vec = bounding.to_vec();
    vec.push(vec.first().unwrap().clone());
    for p in vec.windows(2) {
        imgproc::line(
            frame,
            core::Point2i::new(p[0].x as i32, p[0].y as i32),
            core::Point2i::new(p[1].x as i32, p[1].y as i32),
            core::VecN::new(0.0, 255.0, 0.0, 1.0),
            2,
            imgproc::LINE_8,
            0,
        )?;
    }

    Ok(())
}

pub fn draw_x(frame: &mut Mat, p: core::Point2i) -> Result<(), Box<dyn std::error::Error>> {
    imgproc::line(
        frame,
        core::Point2i::new(p.x, p.y + 10),
        core::Point2i::new(p.x, p.y - 10),
        core::VecN::new(0.0, 0.0, 255.0, 1.0),
        2,
        imgproc::LINE_8,
        0,
    )?;
    imgproc::line(
        frame,
        core::Point2i::new(p.x + 10, p.y),
        core::Point2i::new(p.x - 10, p.y),
        core::VecN::new(0.0, 0.0, 255.0, 1.0),
        2,
        imgproc::LINE_8,
        0,
    )?;
    Ok(())
}

pub fn avg_corners(bounding: &types::VectorOfPoint2f) -> core::Point2i {
    return core::Point2i::new(
        (bounding.iter().map(|s| s.x).sum::<f32>() / bounding.len() as f32).round() as i32,
        (bounding.iter().map(|s| s.y).sum::<f32>() as f32 / bounding.len() as f32).round() as i32,
    );
}

pub fn bounding_to_rect(bounding: &types::VectorOfPoint2f) -> core::Rect2i {
    let sx = bounding.iter().map(|s| s.x as i32).min().unwrap();
    let sy = bounding.iter().map(|s| s.y as i32).min().unwrap();
    let ex = bounding.iter().map(|s| s.x as i32).max().unwrap();
    let ey = bounding.iter().map(|s| s.y as i32).max().unwrap();

    core::Rect2i::new(sx, sy, (sx - ex).abs(), (sy - ey).abs())
}

pub fn rect(frame: &mut Mat, rect: core::Rect2i) -> Result<(), Box<dyn std::error::Error>> {
    opencv::imgproc::rectangle(
        frame,
        rect,
        core::VecN::new(0.0, 255.0, 255.0, 1.0),
        2,
        opencv::imgproc::LINE_8,
        0,
    )?;
    Ok(())
}

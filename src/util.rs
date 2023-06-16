use camloc_common::opencv::{self, core, imgproc, prelude::*, types};

#[allow(dead_code)]
pub enum Color {
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
}

fn get_color(c: &Color) -> core::Scalar {
    match c {
        Color::Red => core::Scalar::new(0.0, 0.0, 255.0, 1.0),
        Color::Green => core::Scalar::new(0.0, 255.0, 0.0, 1.0),
        Color::Blue => core::Scalar::new(255.0, 255.0, 0.0, 1.0),
        Color::Yellow => core::Scalar::new(255.0, 255.0, 0.0, 1.0),
        Color::Cyan => core::Scalar::new(0.0, 255.0, 255.0, 1.0),
        Color::Magenta => core::Scalar::new(255.0, 0.0, 255.0, 1.0),
    }
}

pub trait Center {
    fn center(&self) -> core::Point2i;
}

impl Center for core::Rect {
    fn center(&self) -> core::Point2i {
        core::Point2i {
            x: self.x + (self.width / 2),
            y: self.y + (self.height / 2),
        }
    }
}

pub fn draw_bounds(
    frame: &mut Mat,
    bounding: &types::VectorOfPoint2f,
    c: Color,
) -> opencv::Result<()> {
    if bounding.len() < 4 {
        return Ok(());
    }

    let mut vec = bounding.to_vec();
    vec.push(
        *vec.first()
            .ok_or(opencv::Error::new(core::StsVecLengthErr, "Wut du heell?"))?,
    );
    for p in vec.windows(2) {
        imgproc::line(
            frame,
            core::Point2i::new(p[0].x as i32, p[0].y as i32),
            core::Point2i::new(p[1].x as i32, p[1].y as i32),
            get_color(&c),
            2,
            imgproc::LINE_8,
            0,
        )?;
    }

    Ok(())
}

pub fn draw_x(frame: &mut Mat, p: core::Point2i, c: Color) -> opencv::Result<()> {
    imgproc::line(
        frame,
        core::Point2i::new(p.x, p.y + 10),
        core::Point2i::new(p.x, p.y - 10),
        get_color(&c),
        2,
        imgproc::LINE_8,
        0,
    )?;
    imgproc::line(
        frame,
        core::Point2i::new(p.x + 10, p.y),
        core::Point2i::new(p.x - 10, p.y),
        get_color(&c),
        2,
        imgproc::LINE_8,
        0,
    )
}

pub fn avg_corners(bounding: &types::VectorOfPoint2f) -> core::Point2i {
    core::Point2i::new(
        (bounding.iter().map(|s| s.x).sum::<f32>() / bounding.len() as f32).round() as i32,
        (bounding.iter().map(|s| s.y).sum::<f32>() / bounding.len() as f32).round() as i32,
    )
}

pub fn bounding_to_rect(bounding: &types::VectorOfPoint2f, offset: i32) -> core::Rect2i {
    let (mut sx, mut sy, mut ex, mut ey) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in bounding {
        sx = sx.min(p.x as i32);
        sy = sy.min(p.y as i32);
        ex = ex.max(p.x as i32);
        ey = ey.max(p.y as i32);
    }

    core::Rect2i::new(
        sx - offset,
        sy - offset,
        ex - sx + (2 * offset),
        ey - sy + (2 * offset),
    )
}

pub fn rect(frame: &mut Mat, rect: core::Rect2i, c: Color) -> opencv::Result<()> {
    imgproc::rectangle(frame, rect, get_color(&c), 2, imgproc::LINE_8, 0)
}

pub fn relative_x(frame: &Mat, point: core::Point2i) -> opencv::Result<f64> {
    Ok(point.x as f64 / frame.size()?.width as f64)
}

use crate::calc::Coordinates;

pub trait Lerp {
	fn lerp(s: &Self, e: &Self, t: f64) -> Self;
}

impl Lerp for f64 {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        (1. - t) * s + t * e
    }
}

impl Lerp for Coordinates {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        Coordinates::new(
			f64::lerp(&s.x, &e.x, t),
			f64::lerp(&s.y, &e.y, t),
		)
    }
}

use crate::calc::Coordinate;
use std::fmt as fmt;

pub trait Lerp {
	fn lerp(s: &Self, e: &Self, t: f64) -> Self;
}

impl Lerp for f64 {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        (1. - t) * s + t * e
    }
}

impl Lerp for Coordinate {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        Coordinate::new(
			f64::lerp(&s.x, &e.x, t),
			f64::lerp(&s.y, &e.y, t),
		)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GenerationalValue<T> {
    generation: usize,
    value: T,
}
impl<T> GenerationalValue<T> {
    pub fn new(value: T) -> Self {
        Self { generation: 0, value }
    }
    pub fn new_with_generation(value: T, generation: usize) -> Self {
        Self {
            generation,
            value,
        }
    }

    pub fn set(&mut self, value: T) {
        self.generation += 1;
        self.value = value;
    }
    pub fn generation(&self) -> usize {
        self.generation
    }
}
impl<T: fmt::Display> fmt::Display for GenerationalValue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} of {}]", self.value, self.generation)
    }
}

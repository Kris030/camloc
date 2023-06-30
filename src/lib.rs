use std::{
    fmt::Display,
    time::{Duration, Instant},
};

pub use camloc_common::{hosts::constants::MAIN_PORT, Position};

mod calc;
pub mod compass;
pub mod extrapolations;
pub mod service;

#[derive(Clone, Copy)]
pub enum MotionHint {
    MovingForwards,
    MovingBackwards,
    Stationary,
}

#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(
    feature = "roblib-parsing",
    derive(roblib_macro::Readable, roblib_macro::Writable)
)]
pub struct PlacedCamera {
    /// Horizontal FOV (**in radians**)
    pub fov: f64,
    pub position: Position,
}

impl PlacedCamera {
    pub fn new(position: Position, fov: f64) -> Self {
        Self { position, fov }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimedPosition {
    pub position: Position,
    start_time: Instant,
    pub time: Instant,

    /// - None - not interpolated
    /// - Some(d) - interpolated by d time
    pub extrapolated_by: Option<Duration>,
}

impl Display for TimedPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pos = &self.position;
        let t = self.time - self.start_time;

        if let Some(from) = self.extrapolated_by {
            write!(f, "[{pos} @ {from:.2?} -> {t:.2?}]")
        } else {
            write!(f, "[{pos} @ {t:.2?}]")
        }
    }
}

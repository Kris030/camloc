use crate::TimedPosition;
use camloc_common::{Lerp, Position};
use std::time::Instant;

pub trait Extrapolation: Send + Sync {
    fn add_datapoint(&mut self, position: TimedPosition);
    fn get_last_datapoint(&self) -> Option<TimedPosition>;
    fn extrapolate(&self, time: Instant) -> Option<Position>;
}

#[derive(Debug, Default)]
pub struct LinearExtrapolation {
    data: [Option<TimedPosition>; 2],
    p: usize,
}

impl LinearExtrapolation {
    pub fn new() -> Self {
        LinearExtrapolation {
            data: [None; 2],
            p: 0,
        }
    }
}

impl Extrapolation for LinearExtrapolation {
    fn add_datapoint(&mut self, position: TimedPosition) {
        self.data[self.p] = Some(position);
        self.p = (self.p + 1) % self.data.len();
    }

    fn extrapolate(&self, to: Instant) -> Option<Position> {
        let p_prev = if self.p == 0 {
            self.data.len() - 1
        } else {
            self.p - 1
        };

        let Some(d1) = self.data[self.p] else {
            return None;
        };
        let Some(d2) = self.data[p_prev] else {
            return None;
        };

        let td = to - d1.time;
        let tmax = d2.time - d1.time;
        let t = td.as_secs_f64() / tmax.as_secs_f64();

        Some(Position::lerp(&d1.position, &d2.position, t))
    }

    fn get_last_datapoint(&self) -> Option<TimedPosition> {
        self.data[self.p]
    }
}

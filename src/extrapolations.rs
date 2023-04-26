use camloc_common::{position::Position, Lerp};
use crate::{service::TimedPosition};
use std::time::{Instant, Duration};

pub trait Extrapolator: Send + Sync {
    fn add_datapoint(&mut self, position: TimedPosition);
	fn get_last_datapoint(&self) -> Option<TimedPosition>;
    fn extrapolate(&self, time: Instant) -> Option<Position>;
}

pub struct Extrapolation {
    pub extrapolator: Box<dyn Extrapolator>,
    pub invalidate_after: Duration,
}

impl Extrapolation {
	pub fn new<E: Extrapolator + Default + 'static>(invalidate_after: Duration) -> Self {
		Extrapolation {
			extrapolator: Box::<E>::default(),
			invalidate_after,
		}
	}
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
			p: 0
		}
	}
}

impl Extrapolator for LinearExtrapolation {
    fn add_datapoint(&mut self, position: TimedPosition) {
		self.data[self.p] = Some(position);
		self.p = (self.p + 1) % self.data.len();
    }

    fn extrapolate(&self, time: Instant) -> Option<Position> {
		let p_prev = if self.p == 0 {
			self.data.len() - 1
		} else {
			self.p - 1
		};

        let Some(d1) = self.data[self.p] else { return None; };
        let Some(d2) = self.data[p_prev] else { return None; };

		let td = time - d1.time;
		let tmax = d2.time - d1.time;
		let t = td.as_secs_f64() / tmax.as_secs_f64();

		Some(Position::lerp(&d1.position, &d2.position, t))
    }

    fn get_last_datapoint(&self) -> Option<TimedPosition> {
        self.data[self.p]
    }
}

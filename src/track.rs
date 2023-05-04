use opencv::video::Tracker;
use opencv::{
    core::{Ptr, Rect},
    prelude::*,
    tracking,
};

use crate::util::{self, Center, Color};

pub struct Tracking {
    tracker: Ptr<dyn tracking::TrackerKCF>,
    /// bounding box of the tracked area
    pub rect: Rect,
}

impl Tracking {
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            tracker: Tracking::reinit(&Mat::default(), Rect::default())?,
            rect: Rect::default(),
        })
    }

    /// creates a new `TrackerKCF` struct (because calling `init` on the same instance causes segfaults for whatever reason)
    fn reinit(frame: &Mat, rect: Rect) -> Result<Ptr<dyn tracking::TrackerKCF>, &'static str> {
        let mut tracker = <dyn tracking::TrackerKCF>::create(
            tracking::TrackerKCF_Params::default()
                .map_err(|_| "Couldn't get default tracker params")?,
        )
        .map_err(|_| "Couldn't create tracker")?;
        if !rect.empty() {
            tracker
                .init(frame, rect)
                .map_err(|_| "Couldn't init tracker")?
        }
        Ok(tracker)
    }

    pub fn init(&mut self, frame: &Mat) -> Result<(), &'static str> {
        self.tracker = Tracking::reinit(frame, self.rect).map_err(|_| "Couldn't init tracker")?;
        Ok(())
    }

    /// returns None if lost object
    pub fn track(
        &mut self,
        frame: &Mat,
        draw: Option<&mut Mat>,
    ) -> Result<Option<f64>, &'static str> {
        match self.tracker.update(&frame, &mut self.rect) {
            Ok(true) => {
                if let Some(draw) = draw {
                    util::rect(draw, self.rect, Color::Cyan).map_err(|_| "Couldn't draw rect")?;
                    util::draw_x(draw, self.rect.center(), Color::Red)
                        .map_err(|_| "Couldn't draw center")?;
                }
                Ok(Some(util::relative_x(frame, self.rect.center())))
            }
            Ok(false) => Ok(None),
            Err(_) => Err("Couldn't update tracker"),
        }
    }
}

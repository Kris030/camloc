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
    pub fn new() -> opencv::Result<Self> {
        Ok(Self {
            tracker: Tracking::reinit(&Mat::default(), Rect::default())?,
            rect: Rect::default(),
        })
    }

    /// creates a new `TrackerKCF` struct (because calling `init` on the same instance causes segfaults for whatever reason)
    fn reinit(frame: &Mat, rect: Rect) -> opencv::Result<Ptr<dyn tracking::TrackerKCF>> {
        let mut tracker =
            <dyn tracking::TrackerKCF>::create(tracking::TrackerKCF_Params::default().unwrap())?;
        if !rect.empty() {
            tracker.init(frame, rect)?;
        }
        Ok(tracker)
    }

    pub fn init(&mut self, frame: &Mat) -> opencv::Result<()> {
        self.tracker = Tracking::reinit(frame, self.rect)?;
        Ok(())
    }

    /// returns None if lost object
    pub fn track(&mut self, frame: &Mat, draw: Option<&mut Mat>) -> opencv::Result<Option<f64>> {
        match self.tracker.update(&frame, &mut self.rect) {
            Ok(true) => {
                if let Some(draw) = draw {
                    util::rect(draw, self.rect, Color::Cyan)?;
                    util::draw_x(draw, self.rect.center(), Color::Red)?;
                }
                Ok(Some(util::relative_x(frame, self.rect.center())))
            }
            _ => Ok(None),
        }
    }
}

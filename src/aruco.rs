use opencv::{
    core::{self, Ptr, Rect},
    objdetect,
    prelude::*,
    tracking, types,
    video::Tracker as CVTracker,
};

use crate::util::{self, Center, Color};
use camloc_common::{cv::get_aruco_dictionary, hosts::ClientData};

pub struct Detector {
    detector: objdetect::ArucoDetector,
    corners: types::VectorOfVectorOfPoint2f,
    rejected: types::VectorOfVectorOfPoint2f,
    marker_ids: core::Vector<i32>,
    cube: [u8; 4],
}

impl Detector {
    /// setup new aruco detector
    /// generate targets with: https://chev.me/arucogen/
    pub fn new(cube: [u8; 4]) -> Result<Self, &'static str> {
        Ok(Self {
            detector: objdetect::ArucoDetector::new(
                &get_aruco_dictionary().map_err(|_| "Couldn't predefined aruco dictionary")?,
                &objdetect::DetectorParameters::default()
                    .map_err(|_| "Couldn't get default aruco detector parameters")?,
                objdetect::RefineParameters {
                    min_rep_distance: 0.5,
                    error_correction_rate: 1.0,
                    check_all_orders: true,
                },
            )
            .map_err(|_| "Couldn't create aruco detector")?,
            corners: types::VectorOfVectorOfPoint2f::new(),
            rejected: types::VectorOfVectorOfPoint2f::new(),
            marker_ids: core::Vector::new(),
            cube,
        })
    }

    pub fn detect(
        &mut self,
        frame: &mut Mat,
        rect: Option<&mut core::Rect>,
        draw: Option<&mut Mat>,
    ) -> Result<Option<ClientData>, &'static str> {
        self.detector
            .detect_markers(
                frame,
                &mut self.corners,
                &mut self.marker_ids,
                &mut self.rejected,
            )
            .map_err(|_| "Couldn't detect markers")?;

        let Some((index, marker_id)) = self.marker_ids.iter()
            .enumerate()
            .find(|(_, s)| self.cube.contains(&(*s as u8))) else {
            return Ok(None);
        };
        let marker_id = marker_id as u8;

        let bounding = self
            .corners
            .get(index)
            .map_err(|_| "Couldn't get target corners")?;
        let center = util::avg_corners(&bounding);
        let brect = util::bounding_to_rect(&bounding, 0).ok_or("No bounding rect?")?;

        if let Some(rect) = rect {
            rect.clone_from(&brect);
        }

        if let Some(draw) = draw {
            util::draw_bounds(draw, &bounding, Color::Green).map_err(|_| "Couldn't draw bounds")?;
            util::draw_x(draw, center, Color::Red).map_err(|_| "Couldn't draw center")?;
            util::rect(draw, brect, Color::Yellow).map_err(|_| "Couldn't draw rectangle")?;
        }

        Ok(Some(ClientData {
            marker_id,
            target_x_position: util::relative_x(frame, center),
        }))
    }
}

pub struct Tracker {
    tracker: Ptr<dyn tracking::TrackerKCF>,
    /// bounding box of the tracked area
    pub rect: Rect,
}

impl Tracker {
    pub fn new() -> Result<Self, &'static str> {
        Ok(Self {
            tracker: Self::reinit(&Mat::default(), Rect::default())?,
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
        self.tracker = Self::reinit(frame, self.rect).map_err(|_| "Couldn't init tracker")?;
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

pub struct Aruco {
    tracked_object: Option<ClientData>,
    detector: Detector,
    tracker: Tracker,
    // TODO: sanity checks
}

impl Aruco {
    pub fn new(cube: [u8; 4]) -> Result<Aruco, &'static str> {
        Ok(Self {
            detector: Detector::new(cube)?,
            tracker: Tracker::new()?,
            tracked_object: None,
        })
    }

    pub fn detect(
        &mut self,
        frame: &mut Mat,
        draw: Option<&mut Mat>,
    ) -> Result<Option<ClientData>, &'static str> {
        self.tracked_object = if let Some(ClientData { marker_id, .. }) = self.tracked_object {
            self.tracker.track(frame, draw)?.map(|x| ClientData {
                marker_id,
                target_x_position: x,
            })
        } else {
            let res = self
                .detector
                .detect(frame, Some(&mut self.tracker.rect), draw)?;
            if res.is_some() {
                self.tracker.init(frame)?;
            }
            res
        };
        Ok(self.tracked_object)
    }
}

use camloc_common::opencv::{
    self,
    core::{self, Ptr, Rect},
    objdetect,
    prelude::*,
    tracking::{self, TrackerKCF},
    types,
    video::TrackerTrait,
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
    pub fn new(cube: [u8; 4]) -> opencv::Result<Self> {
        Ok(Self {
            detector: objdetect::ArucoDetector::new(
                &get_aruco_dictionary()?,
                &objdetect::DetectorParameters::default()?,
                objdetect::RefineParameters {
                    min_rep_distance: 0.5,
                    error_correction_rate: 1.0,
                    check_all_orders: true,
                },
            )?,
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
    ) -> opencv::Result<Option<ClientData>> {
        self.detector.detect_markers(
            frame,
            &mut self.corners,
            &mut self.marker_ids,
            &mut self.rejected,
        )?;

        let Some((index, marker_id)) = self.marker_ids.iter()
            .enumerate()
            .find(|(_, s)| self.cube.contains(&(*s as u8))) else {
            return Ok(None);
        };
        let marker_id = marker_id as u8;

        let bounding = self.corners.get(index)?;
        let center = util::avg_corners(&bounding);
        let brect = util::bounding_to_rect(&bounding, 0);

        if let Some(rect) = rect {
            rect.clone_from(&brect);
        }

        if let Some(draw) = draw {
            util::draw_bounds(draw, &bounding, Color::Green)?;
            util::draw_x(draw, center, Color::Red)?;
            util::rect(draw, brect, Color::Yellow)?;
        }

        Ok(Some(ClientData {
            marker_id,
            x_position: util::relative_x(frame, center)?,
        }))
    }
}

pub struct Tracker {
    kcf: Ptr<TrackerKCF>,
    /// bounding box of the tracked area
    pub rect: Rect,
}

impl Tracker {
    pub fn new() -> opencv::Result<Self> {
        Ok(Self {
            kcf: Self::reinit(&Mat::default(), Rect::default())?,
            rect: Rect::default(),
        })
    }

    /// creates a new `TrackerKCF` struct (because calling `init` on the same instance causes segfaults for whatever reason)
    fn reinit(frame: &Mat, rect: Rect) -> opencv::Result<Ptr<TrackerKCF>> {
        let mut tracker = TrackerKCF::create(tracking::TrackerKCF_Params::default()?)?;
        if !rect.empty() {
            tracker.init(frame, rect)?
        }
        Ok(tracker)
    }

    pub fn init(&mut self, frame: &Mat) -> opencv::Result<()> {
        self.kcf = Self::reinit(frame, self.rect)?;
        Ok(())
    }

    /// returns None if lost object
    pub fn track(&mut self, frame: &Mat, draw: Option<&mut Mat>) -> opencv::Result<Option<f64>> {
        if self.kcf.update(&frame, &mut self.rect)? {
            if let Some(draw) = draw {
                util::rect(draw, self.rect, Color::Cyan)?;
                util::draw_x(draw, self.rect.center(), Color::Red)?;
            }

            Ok(Some(util::relative_x(frame, self.rect.center())?))
        } else {
            Ok(None)
        }
    }
}

pub struct Aruco {
    tracked_object: Option<ClientData>,
    detector: Detector,
    tracker: Tracker,
    // THINKME: sanity checks
}

impl Aruco {
    pub fn new(cube: [u8; 4]) -> opencv::Result<Aruco> {
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
    ) -> opencv::Result<Option<ClientData>> {
        self.tracked_object = if let Some(ClientData { marker_id, .. }) = self.tracked_object {
            self.tracker.track(frame, draw)?.map(|x| ClientData {
                marker_id,
                x_position: x,
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

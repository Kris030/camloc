use crate::util;
use opencv::{core, objdetect, prelude::*, types};

pub struct Aruco {
    detector: objdetect::ArucoDetector,
    corners: types::VectorOfVectorOfPoint2f,
    rejected: types::VectorOfVectorOfPoint2f,
    marker_ids: core::Vector<i32>,
    aruco_target: i32,
}

impl Aruco {
    /// setup new aruco detector
    /// generate targets with: https://chev.me/arucogen/
    pub fn new(aruco_target: i32) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            detector: objdetect::ArucoDetector::new(
                &objdetect::get_predefined_dictionary(
                    objdetect::PredefinedDictionaryType::DICT_4X4_50,
                )?,
                &objdetect::DetectorParameters::default()?,
                objdetect::RefineParameters {
                    min_rep_distance: 0.5,
                    error_correction_rate: 1.0,
                    check_all_orders: false,
                },
            )?,
            corners: opencv::types::VectorOfVectorOfPoint2f::new(),
            rejected: opencv::types::VectorOfVectorOfPoint2f::new(),
            marker_ids: core::Vector::<i32>::new(),
            aruco_target,
        })
    }

    pub fn detect(&mut self, frame: &mut Mat) -> Result<Option<i32>, Box<dyn std::error::Error>> {
        self.detector.detect_markers(
            frame,
            &mut self.corners,
            &mut self.marker_ids,
            &mut self.rejected,
        )?;

        let Some(index) = self.marker_ids.iter().position(|s| s == self.aruco_target) else {
            return Ok(None);
        };

        let bounding = self.corners.get(index).unwrap();
        let center = util::avg_corners(&bounding);
        let rect = util::bounding_to_rect(&bounding);

        util::draw_rect(frame, &bounding)?;
        util::draw_x(frame, center)?;
        util::rect(frame, rect)?;

        Ok(Some(center.x))
    }
}

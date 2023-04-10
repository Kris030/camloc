use crate::util::{self, Color};
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
    pub fn new(aruco_target: i32) -> opencv::Result<Self> {
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

    pub fn detect(
        &mut self,
        frame: &mut Mat,
        rect: Option<&mut core::Rect>,
        draw: Option<&mut Mat>,
    ) -> Result<Option<f64>, Box<dyn std::error::Error>> {
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
        let brect = util::bounding_to_rect(&bounding, 0);

        if let Some(rect) = rect {
            rect.clone_from(&brect);
        }

        if let Some(draw) = draw {
            util::draw_bounds(draw, &bounding, Color::Green)?;
            util::draw_x(draw, center, Color::Red)?;
            util::rect(draw, brect, Color::Yellow)?;
        }

        Ok(Some(util::relative_x(&frame, center)))
    }
}

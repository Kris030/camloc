use crate::track::Tracking;
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
                    check_all_orders: true,
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
    ) -> opencv::Result<Option<f64>> {
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

pub fn detect(
    frame: &mut Mat,
    draw: Option<&mut Mat>,
    has_object: &mut bool,
    aruco: &mut Aruco,
    tracker: &mut Tracking,
) -> opencv::Result<f64> {
    let mut final_x = f64::NAN;
    if !*has_object {
        if let Some(x) = aruco.detect(frame, Some(&mut tracker.rect), draw)? {
            final_x = x;
            *has_object = true;
            tracker.init(&frame)?;
        }
    } else {
        if let Some(x) = tracker.track(&frame, draw)? {
            final_x = x;
        } else {
            *has_object = false;
        }
    }

    Ok(final_x)
}

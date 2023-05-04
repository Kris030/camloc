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
    pub fn new(aruco_target: i32) -> Result<Self, &'static str> {
        Ok(Self {
            detector: objdetect::ArucoDetector::new(
                &objdetect::get_predefined_dictionary(
                    objdetect::PredefinedDictionaryType::DICT_4X4_50,
                )
                .map_err(|_| "Couldn't predefined aruco dictionary")?,
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
            aruco_target,
        })
    }

    pub fn detect(
        &mut self,
        frame: &mut Mat,
        rect: Option<&mut core::Rect>,
        draw: Option<&mut Mat>,
    ) -> Result<Option<f64>, &'static str> {
        self.detector
            .detect_markers(
                frame,
                &mut self.corners,
                &mut self.marker_ids,
                &mut self.rejected,
            )
            .map_err(|_| "Couldn't detect markers")?;

        let Some(index) = self.marker_ids.iter().position(|s| s == self.aruco_target) else {
            return Ok(None);
        };

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

        Ok(Some(util::relative_x(frame, center)))
    }
}

pub fn detect(
    frame: &mut Mat,
    draw: Option<&mut Mat>,
    has_object: &mut bool,
    aruco: &mut Aruco,
    tracker: &mut Tracking,
) -> Result<f64, &'static str> {
    let mut final_x = f64::NAN;
    if !*has_object {
        if let Some(x) = aruco.detect(frame, Some(&mut tracker.rect), draw)? {
            final_x = x;
            *has_object = true;
            tracker.init(frame)?;
        }
    } else if let Some(x) = tracker.track(frame, draw)? {
        final_x = x;
    } else {
        *has_object = false;
    }

    Ok(final_x)
}

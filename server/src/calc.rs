use camloc_common::{hosts::ClientData, Position};

use crate::{MotionHint, PlacedCamera};

#[allow(clippy::needless_range_loop)]
pub fn calculate_position(
    min_camera_angle_diff: f64,
    data: &[(Option<ClientData>, PlacedCamera)],
    motion_data: Option<MotionData>,
    compass_data: Option<f64>,
    last_position: Option<Position>,
    _cube: [u8; 4],
) -> Option<Position> {
    let c = data.len();

    let mut tangents = vec![None; c];

    let mut lines = 0u32;
    for i in 0..c {
        if let (Some(data), camera) = data[i] {
            let tan = (camera.position.rotation + (camera.fov * (0.5 - data.x_position))).tan();
            tangents[i] = Some(tan);
            lines += 1;
        }
    }
    if lines < 2 {
        return None;
    }

    let (mut x, mut y) = (0., 0.);
    let mut points = 0usize;

    for i in 0..c {
        let Some(atan) = tangents[i] else {
            continue;
        };
        let c1 = data[i].1.position;
        let a1 = c1.rotation % 180.;

        for j in 0..i {
            let Some(btan) = tangents[j] else {
                continue;
            };

            let c2 = data[j].1.position;
            let a2 = c2.rotation % 180.;

            let diff = (a1 - a2).abs();
            let diff = diff.min(180. - diff);
            if diff < min_camera_angle_diff {
                continue;
            }

            let px = (c1.x * atan - c2.x * btan - c1.y + c2.y) / (atan - btan);
            let py = atan * (px - c1.x) + c1.y;

            x += px;
            y += py;

            points += 1;
        }
    }

    let points = points as f64;

    x /= points;
    y /= points;

    let comp_rot = compass_data;
    let pos_rot = get_pos_based_rotation(x, y, motion_data, last_position);

    // TODO: improve calculation (increase weight of position based)
    let (mut r, mut rc) = (0., 0u64);

    for rot in [comp_rot, pos_rot].into_iter().flatten() {
        r += rot;
        rc += 1;
    }
    let r = if rc == 0 { f64::NAN } else { r / rc as f64 };

    Some(Position::new(x, y, r))
}

fn get_pos_based_rotation(
    x: f64,
    y: f64,
    motion_data: Option<MotionData>,
    last_position: Option<Position>,
) -> Option<f64> {
    let Some(data) = motion_data else {
        return None;
    };
    let Some(last_position) = &last_position else {
        return None;
    };

    let rot_dir = match data.hint {
        MotionHint::MovingBackwards => -1.,
        MotionHint::MovingForwards => 1.,
        MotionHint::Stationary => return Some(data.last_moving_position.rotation),
    };

    Some(rot_dir * f64::atan2(x - last_position.x, y - last_position.y))
}

#[derive(Clone, Copy)]
pub struct MotionData {
    pub last_moving_position: Position,
    pub hint: MotionHint,
}

impl MotionData {
    pub fn new(last_moving_position: Position, hint: MotionHint) -> Self {
        Self {
            last_moving_position,
            hint,
        }
    }
}

use camloc_common::{hosts::ClientData, Position};

use crate::{MotionHint, PlacedCamera};

pub fn calculate_position(position_data: &PositionData) -> Option<Position> {
    let c = position_data.data.len();

    let mut tangents = vec![None; c];

    let mut lines = 0u32;
    #[allow(clippy::needless_range_loop)]
    for i in 0..c {
        if let (Some(data), camera) = position_data.data[i] {
            tangents[i] =
                Some((camera.position.rotation + (camera.fov * (0.5 - data.x_position))).tan());
            lines += 1;
        }
    }
    if lines < 2 {
        return None;
    }

    let (mut x, mut y) = (0., 0.);
    let points = ((lines * (lines - 1)) / 2) as f64;

    for i in 0..c {
        for j in (i + 1)..c {
            let Some(atan) = tangents[i] else { continue; };
            let Some(btan) = tangents[j] else { continue; };

            let c1 = position_data.data[i].1.position;
            let c2 = position_data.data[j].1.position;

            let px = (c1.x * atan - c2.x * btan - c1.y + c2.y) / (atan - btan);
            let py = atan * (px - c1.x) + c1.y;

            x += px / points;
            y += py / points;
        }
    }

    let comp_rot = position_data.compass_data;
    let pos_rot = get_pos_based_rotation(x, y, position_data);
    // TODO: improve calculation
    let (mut r, mut rc) = (0., 0u64);

    for rot in [comp_rot, pos_rot].iter().flatten() {
        r += rot;
        rc += 1;
    }
    let r = if rc == 0 { f64::NAN } else { r / rc as f64 };

    Some(Position::new(x, y, r))
}

fn get_pos_based_rotation(x: f64, y: f64, position_data: &PositionData) -> Option<f64> {
    let Some(data) = position_data.motion_data else { return None; };
    let rot_dir = match data.hint {
        MotionHint::MovingBackwards => -1.,
        MotionHint::MovingForwards => 1.,
        MotionHint::Stationary => return Some(data.last_moving_position.rotation),
    };

    Some(
        rot_dir
            * f64::atan2(
                x - position_data.last_position.x,
                y - position_data.last_position.y,
            ),
    )
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

pub struct PositionData<'a> {
    pub data: &'a [(Option<ClientData>, PlacedCamera)],
    pub motion_data: Option<MotionData>,
    pub compass_data: Option<f64>,
    pub last_position: Position,
    pub cube: [u8; 4],
}

impl<'a> PositionData<'a> {
    pub fn new(
        data: &'a [(Option<ClientData>, PlacedCamera)],
        motion_data: Option<MotionData>,
        compass_data: Option<f64>,
        last_position: Position,
        cube: [u8; 4],
    ) -> Self {
        Self {
            last_position,
            compass_data,
            motion_data,
            data,
            cube,
        }
    }
}

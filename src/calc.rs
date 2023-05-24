use camloc_common::{hosts::ClientData, position::Position};

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlacedCamera {
    /// Horizontal FOV (**in radians**)
    pub fov: f64,
    pub position: Position,
}

impl PlacedCamera {
    pub fn new(position: Position, fov: f64) -> Self {
        Self { position, fov }
    }
}

pub struct Setup {
    pub cameras: Vec<PlacedCamera>,
}

impl Setup {
    pub fn new() -> Self {
        Self { cameras: vec![] }
    }

    pub fn calculate_position(&self, position_data: PositionData) -> Option<Position> {
        let c = self.cameras.len();
        debug_assert_eq!(c, position_data.client_data.len());

        let mut tangents = vec![None; c];

        let mut lines = 0u32;
        #[allow(clippy::needless_range_loop)]
        for i in 0..c {
            if let Some(d) = position_data.client_data[i] {
                tangents[i] = Some(
                    (self.cameras[i].position.rotation
                        + (self.cameras[i].fov * (0.5 - d.target_x_position)))
                        .tan(),
                );
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

                let c1 = self.cameras[i].position;
                let c2 = self.cameras[j].position;

                let px = (c1.x * atan - c2.x * btan - c1.y + c2.y) / (atan - btan);
                let py = atan * (x - c1.x) + c1.y;

                x += px / points;
                y += py / points;
            }
        }

        let comp_rot = position_data.compass_data;
        let pos_rot = Setup::get_pos_based_rotation(x, y, &position_data);
        // TODO: handle clients rot data (?)

        let (mut r, mut rc) = (0., 0u64);
        #[allow(clippy::manual_flatten)]
        for rot in [comp_rot, pos_rot] {
            if let Some(cr) = rot {
                r += cr;
                rc += 1;
            }
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
}

impl Default for Setup {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
pub enum MotionHint {
    MovingForwards,
    MovingBackwards,
    Stationary,
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
    pub client_data: &'a [Option<ClientData>],
    pub motion_data: Option<MotionData>,
    pub compass_data: Option<f64>,
    pub last_position: Position,
    pub cube: [u8; 4],
}

impl<'a> PositionData<'a> {
    pub fn new(
        client_data: &'a [Option<ClientData>],
        motion_data: Option<MotionData>,
        compass_data: Option<f64>,
        last_position: Position,
        cube: [u8; 4],
    ) -> Self {
        Self {
            cube,
            client_data,
            motion_data,
            compass_data,
            last_position,
        }
    }
}

use camloc_common::position::Position;
use std::collections::HashMap;

/// Physical characteristics of a camera
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CameraInfo {
    /// Horizontal FOV (**in radians**)
    pub fov: f64,
}

impl CameraInfo {
    /// FOV is **in radians**
    pub fn new(fov: f64) -> Self {
        Self { fov }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct PlacedCamera {
    pub info: CameraInfo,
    pub position: Position,
}

impl PlacedCamera {
    pub fn new(info: CameraInfo, position: Position) -> Self {
        Self { info, position, }
    }
}

pub struct Setup {
    pub cameras: Vec<PlacedCamera>,
}

impl Setup {
	pub fn new_freehand(cameras: Vec<PlacedCamera>) -> Self {
		Self { cameras }
	}

    pub fn new_square(square_size: f64, cameras: Vec<CameraInfo>) -> Self {
        let c = cameras.len();
        debug_assert!(
            (2..=4).contains(&c),
            "A square setup may only have 2 or 4 cameras"
        );

        let mut hmap: HashMap<u64, f64> = HashMap::new();
        
        let mut ind = 0;
        let cameras = cameras
            .into_iter()
            .map(|c| {
                let bits = c.fov.to_bits();
                let d = match hmap.get(&bits) {
                    Some(v) => *v,
                    None => {
                        let v = camloc_common::position::get_camera_distance_in_square(square_size, c.fov);
                        hmap.insert(c.fov.to_bits(), v);
                        v
                    }
                };

                let info = c;
                let pos = camloc_common::position::calc_posotion_in_square_distance(ind, d);
                ind += 1;

                PlacedCamera::new(
                    info,
                    pos,
                )
            }
        ).collect();

        Self { cameras }
    }

    pub fn calculate_position(&self, pxs: &Vec<Option<f64>>) -> Option<Position> {
        let c = self.cameras.len();
        debug_assert_eq!(c, pxs.len());

        let mut tangents = vec![None; c];

        let mut lines = 0u32;
        for i in 0..c {
            if let Some(x) = pxs[i] {
                tangents[i] = Some(
                    (self.cameras[i].position.rotation + (self.cameras[i].info.fov * (0.5 - x))).tan()
                );
                lines += 1;
            }
        }
        if lines < 2 {
            return None;
        }

        let mut s = Position::new(0., 0., f64::NAN);

        for i in 0..c {
            for j in (i + 1)..c {
                let Some(atan) = tangents[i] else { continue; };
                let Some(btan) = tangents[j] else { continue; };

                let c1 = self.cameras[i].position;
                let c2 = self.cameras[j].position;

                let x = (c1.x * atan - c2.x * btan - c1.y + c2.y) / (atan - btan);
                let y = atan * (x - c1.x) + c1.y;

                s.x += x;
                s.y += y;
            }
        }

        let points = (lines * (lines - 1) / 2) as f64;

        Some(Position::new(s.x / points, s.y / points, f64::NAN))
    }
}

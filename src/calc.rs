use std::{collections::HashMap, fmt::Display};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinates {
    pub x: f64,
    pub y: f64,
}

impl Coordinates {
    pub const fn new(x: f64, y: f64) -> Self {
        Coordinates { x, y }
    }
}

impl From<(f64, f64)> for Coordinates {
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl Display for Coordinates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}; {:.2})", self.x, self.y)
    }
}

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

#[derive(Debug, PartialEq)]
pub struct PlacedCamera {
    pub info: CameraInfo,
    pub pos: Coordinates,

    /// **IN RADIANS**
    pub rot: f64,
}

impl PlacedCamera {
    pub fn new(info: CameraInfo, pos: Coordinates, rot: f64) -> Self {
        Self { info, pos, rot }
    }
}

pub struct Setup {
    pub(crate) cameras: Vec<PlacedCamera>,
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
        let cpos = [
            (-1., 0.),
            (0., -1.),
            (1.,  0.),
            (0., -1.),
        ];
        let mut ind = 0;
        let cameras = cameras
            .into_iter()
            .map(|c| {
                let p = &cpos[ind % 4];

                let bits = c.fov.to_bits();
                let d = match hmap.get(&bits) {
                    Some(v) => *v,
                    None => {
                        let v = 0.5 * square_size * (
                            1. / (
                                0.5 * c.fov
                            ).tan() + 1.
                        );
                        hmap.insert(c.fov.to_bits(), v);
                        v
                    }
                };

                let info = c;
                let pos = Coordinates::new(p.0 * d, p.1 * d);
                let rot = (ind as f64) * 90.;
                ind += 1;

                PlacedCamera::new(
                    info,
                    pos,
                    rot.to_radians()
                )
            }
        ).collect();

        Self { cameras }
    }

    pub fn calculate_position(&self, pxs: Vec<Option<f64>>) -> Option<Coordinates> {
        let c = self.cameras.len();
        debug_assert_eq!(c, pxs.len());

        let mut tangents = vec![None; c];

        let mut lines = 0u32;
        for i in 0..c {
            if let Some(x) = pxs[i] {
                tangents[i] = Some(
                    (self.cameras[i].rot + (self.cameras[i].info.fov * (0.5 - x))).tan()
                );
                lines += 1;
            }
        }
        if lines < 2 {
            return None;
        }

        let mut s = Coordinates::new(0., 0.);

        for i in 0..c {
            for j in (i + 1)..c {
                let Some(atan) = tangents[i] else { continue; };
                let Some(btan) = tangents[j] else { continue; };

                let c1 = self.cameras[i].pos;
                let c2 = self.cameras[j].pos;

                let x = (c1.x * atan - c2.x * btan - c1.y + c2.y) / (atan - btan);
                let y = atan * (x - c1.x) + c1.y;

                s.x += x;
                s.y += y;
            }
        }

        let points = (lines * (lines - 1) / 2) as f64;

        Some(Coordinates::new(s.x / points, s.y / points))
    }

    pub fn cameras(&self) -> &[PlacedCamera] {
        &self.cameras
    }

    pub fn camera_count(&self) -> usize {
        self.cameras.len()
    }
}

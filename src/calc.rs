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
    /// Horizontal, vertical FOV (**in radians**)
    pub fov: (f64, f64),
}

impl CameraInfo {
    /// FOV **in degrees**
    pub fn new(fov: (f64, f64)) -> Self {
        Self {
            fov: (fov.0.to_radians(), fov.1.to_radians())
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct PlacedCamera {
    info: CameraInfo,
    pos: Coordinates,

    /// **IN RADIANS**
    rot: f64,
}

impl PlacedCamera {
    pub fn new(info: CameraInfo, pos: Coordinates, rot: f64) -> Self {
        Self { info, pos, rot }
    }
}

pub struct Setup<const C: usize> {
    cameras: [PlacedCamera; C],
}

impl<const C: usize> Setup<C> {
	pub fn new_freehand(cameras: [PlacedCamera; C]) -> Self {
		Self { cameras }
	}

    pub fn new_square(square_size: f64, cameras: [CameraInfo; C]) -> Self {
        debug_assert!(
            2 <= C && C <= 4,
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
            .map(|c| {
                let p = &cpos[ind % 4];

                let bits = c.fov.0.to_bits();
                let d = match hmap.get(&bits) {
                    Some(v) => *v,
                    None => {
                        let v = 0.5 * square_size * (
                            1. / (
                                0.5 * c.fov.0
                            ).tan() + 1.
                        );
                        hmap.insert(c.fov.0.to_bits(), v);
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
        );

        Self { cameras }
    }

    pub fn calculate_position(&self, pxs: &[Option<f64>; C]) -> Option<Coordinates> {
        let mut tangents = [None; C];

        let mut lines = 0u32;
        for i in 0..C {
            if let Some(x) = pxs[i] {
                tangents[i] = Some(
                    (self.cameras[i].rot + (self.cameras[i].info.fov.0 * (0.5 - x))).tan()
                );
                lines += 1;
            }
        }
        if lines == 0 {
            return None;
        }

        let mut s = Coordinates::new(0., 0.);

        for i in 0..C {
            for j in (i + 1)..C {
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
}

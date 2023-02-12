use std::collections::HashMap;

pub type Position = (f64, f64);

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
    pos: Position,

    /// **IN RADIANS**
    rot: f64,
}

impl PlacedCamera {
    pub fn new(info: CameraInfo, pos: Position, rot: f64) -> Self {
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

                let bits = unsafe { std::mem::transmute::<f64, u64>(c.fov.0) };
                let d = match hmap.get(&bits) {
                    Some(v) => *v,
                    None => {
                        let v = 0.5 * square_size * (
                            1. / (
                                0.5 * c.fov.0
                            ).tan() + 1.
                        );
                        hmap.insert(unsafe { std::mem::transmute(c.fov.0) }, v);
                        v
                    }
                };

                let info = c;
                let pos = (p.0 * d, p.1 * d);
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

    pub fn calculate_position(&self, pxs: &[Option<f64>; C]) -> Option<Position> {
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

        let mut s = (0., 0.);

        for i in 0..C {
            for j in (i + 1)..C {
                let Some(atan) = tangents[i] else { continue; };
                let Some(btan) = tangents[j] else { continue; };

                let c1 = self.cameras[i].pos;
                let c2 = self.cameras[j].pos;

                let x = (c1.0 * atan - c2.0 * btan - c1.1 + c2.1) / (atan - btan);

                let y = atan * (x - c1.0) + c1.1;

                s.0 += x;
                s.1 += y;
            }
        }

        let points = (lines * (lines - 1) / 2) as f64;

        Some((s.0 / points, s.1 / points))
    }
}

use std::collections::HashMap;

pub type Position = (f64, f64);

/// Physical characteristics of a camera
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CameraInfo {
    /// Horizontal, vertical Field Of View
    pub fov: (f64, f64),
}

impl CameraInfo {
    pub fn new(fov: (f64, f64)) -> Self { Self { fov } }
}

#[derive(Debug, PartialEq)]
pub struct PlacedCamera {
    info: CameraInfo,
    pos: Position,
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
        let mut cd = |c: &CameraInfo| {
            if let Some(v) = hmap.get(&unsafe { std::mem::transmute(c.fov.0) }) {
                *v
            } else {
                let v = 0.5 * square_size * (1. + 1. / (0.5 * c.fov.0).tan());
                hmap.insert(unsafe { std::mem::transmute(c.fov.0) }, v);
                v
            }
        };

        let mut cs: [_; C] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        cs[0] = PlacedCamera::new(cameras[0], (-cd(&cameras[0]), 0.), 0f64.to_radians());
        cs[1] = PlacedCamera::new(cameras[1], (0., -cd(&cameras[1])), 90f64.to_radians());

        if C >= 3 {
            cs[2] = PlacedCamera::new(cameras[2], (cd(&cameras[2]), 0.), 180f64.to_radians());
        }
        if C == 4 {
            cs[3] = PlacedCamera::new(cameras[3], (0., cd(&cameras[3])), 270f64.to_radians());
        }

        Self { cameras: cs.into() }
    }

    pub fn calculate_position(&self, pxs: &[Option<f64>; C]) -> Option<Position> {
        let mut tangents = vec![None; C];

        let mut is = 0u32;
        for i in 0..C {
            tangents[i] = if let Some(x) = pxs[i] {
                is += 1;
                Some((self.cameras[i].rot + (self.cameras[i].info.fov.0 * (0.5 - x))).tan())
            } else { None };
        }
        if is == 0 {
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

        Some((s.0 / is as f64, s.1 / is as f64))
    }
}

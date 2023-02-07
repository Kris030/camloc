extern crate uom;

use nalgebra::Vector2;
use uom::si::length::meter;
use uom::si::angle::degree;
use uom::si::ratio::ratio;

use uom::si::f64::{Length, Angle, Ratio};
use uom::ConstZero;

/// Physical characteristics of a camera
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct CameraInfo {

    /// Horizontal, vertical Field Of View
    pub fov: Vector2<Angle>,
    
    // /// Dimensions of the physical camera module
    // pub camera_module_size: (Length, Length, Length),

    // /// Image resolution
    // pub camera_resolution: Vector2<u32>,

    // /// Width and height of the image sensor
    // pub sensor_image_area: Vector2<Length>,
    // /// Length of the image sensor diagonal
    // pub sensor_diagonal: Length,

    // /// Focal length
    // pub focal_length: Length,

    // /// Pixel size
    // pub pixel_size: Vector2<Length>,

    // /// Optical size
    // pub optical_size: Length,
}

fn camera_distance(square_size: Length, c: &CameraInfo) -> Length {
    0.5 * square_size * (
        Ratio::new::<ratio>(1.) +
        1. / (0.5 * c.fov.x).tan()
    )
}

fn calc_square_pos_pair(
    c1: Vector2<Length>, c2: Vector2<Length>,
    fov1: Angle, fov2: Angle,
    px1: f64, px2: f64
) -> Vector2<Length> {
    let alpha: Angle = fov1 * (0.5 - px1);
    let atan: Ratio = alpha.tan();
    
    let beta: Angle = Angle::new::<degree>(90.) - fov2 * (0.5 - px2);
    let btan: Ratio = beta.tan();

    let x: Length = (c1.x * atan - c2.x * btan - c1.y + c2.y)
                        / (atan - btan);

    let y: Length = atan * (x - c1.x) + c1.y;

    Vector2::new(x, y)
}

#[derive(Debug, PartialEq)]
pub struct PlacedCamera {
    info: CameraInfo,
    pos: Vector2<Length>,
    rot: Angle,
}

impl PlacedCamera {
    pub fn new(info: CameraInfo, pos: Vector2<Length>, rot: Angle) -> Self {
        Self { info, pos, rot }
    }
}
pub struct Setup<const C: usize> {
    cameras: [PlacedCamera; C],
}

impl<const C: usize> Setup<C> {
    
    pub fn new_square(square_size: Length, cameras: [CameraInfo; C]) -> Self {
        debug_assert!(2 <= C && C <= 4, "A square setup may only have 2 or 4 cameras");

        use self::camera_distance as cd;

        let mut cs: [_; C] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        cs[0] = PlacedCamera::new(cameras[0],
            Vector2::new(-cd(square_size, &cameras[0]), Length::ZERO),
            Angle::ZERO,
        );
        cs[1] = PlacedCamera::new(cameras[1],
            Vector2::new(Length::ZERO, cd(square_size, &cameras[1])),
            Angle::new::<degree>(90.),
        );

        if C >= 3 {
            cs[2] = PlacedCamera::new(cameras[2],
                Vector2::new(cd(square_size, &cameras[2]), Length::ZERO),
                Angle::new::<degree>(180.),
            );
        }
        if C == 4 {
            cs[3] = PlacedCamera::new(cameras[3],
                Vector2::new(Length::ZERO, -cd(square_size, &cameras[3])),
                Angle::new::<degree>(270.),
            );
        }

        Self { cameras: cs, }
    }
    
    pub fn calculate_position(&self, pxs: [f64; C]) -> Vector2<Length> {
        let mut tangents: [Ratio; C] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };

        for i in 0..C {
            tangents[i] = (self.cameras[i].rot - (self.cameras[i].info.fov.x * (0.5 - pxs[i]))).tan();
        }

        let mut s: Vector2<Length> = Vector2::new(Length::ZERO, Length::ZERO);
        for i in 0..C {
            for j in 0..C {
                if i == j {
                    continue;
                }

                let atan: Ratio = tangents[i];
                let btan: Ratio = tangents[j];

                let c1: Vector2<Length> = self.cameras[i].pos;
                let c2: Vector2<Length> = self.cameras[j].pos;

                let x: Length = (c1.x * atan - c2.x * btan - c1.y + c2.y)
                                    / (atan - btan);

                let y: Length = atan * (x - c1.x) + c1.y;

                s += Vector2::new(x, y);
            }
        }

        s.x /= 
        
        // TODO: remove
		// calc_square_pos_pair(
        //     self.cameras[0].pos, self.cameras[1].pos,
        //     self.cameras[0].info.fov.x, self.cameras[1].info.fov.x,
        //     pxs[0], pxs[1]
        // )
    }
}

fn main() {
	let picamera = CameraInfo {
        fov: Vector2::new( 
            Angle::new::<degree>(62.2),
            Angle::new::<degree>(48.8),
        ),

        // camera_module_size: (
        //     Length::new::<millimeter>(25.0),
        //     Length::new::<millimeter>(24.0),
        //     Length::new::<millimeter>(9.00),
        // ),

        // camera_resolution: (3280, 2464),

        // sensor_image_area: (
        //     Length::new::<millimeter>(3.68),
        //     Length::new::<millimeter>(2.76),
        // ),
        // sensor_diagonal: Length::new::<millimeter>(4.6),

        // focal_length: Length::new::<millimeter>(3.04),

        // pixel_size: (
        //     Length::new::<micrometer>(1.12),
        //     Length::new::<micrometer>(1.12),
        // ),

        // optical_size: Length::new::<inch>(0.25),
    };
    let setup = Setup::new_square(
        Length::new::<meter>(3.0),
        [picamera, picamera],
	);

    println!("Robot position: {:?}",
        setup.calculate_position([0.2.into(), 0.3.into()]),
    );

}

use super::Lerp;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
}

impl Position {
    pub const fn new(x: f64, y: f64, rotation: f64) -> Self {
        Position { x, y, rotation }
    }
}

impl From<(f64, f64, f64)> for Position {
    fn from((x, y, rotation): (f64, f64, f64)) -> Self {
        Self::new(x, y, rotation)
    }
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:.2}; {:.2} {:.2}°)", self.x, self.y, self.rotation)
    }
}

static CPOS: [(f64, f64); 4] = [
    (-1., 0.),
    (0., -1.),
    (1.,  0.),
    (0., -1.),
];

pub fn calc_posotion_in_square_fov(side_length: f64, index: usize, fov: f64) -> Position {
    debug_assert!(
        index < 4,
        "A square setup may only have 2 or 4 cameras"
    );

    let p = &CPOS[index];
    let d = get_camera_distance_in_square(side_length, fov);

    Position::new(
        p.0 * d,
        p.1 * d,
        (index as f64) * 90f64.to_radians(),
    )
}

pub fn calc_posotion_in_square_distance(index: usize, distance: f64) -> Position {
    debug_assert!(
        index < 4,
        "A square setup may only have 2 or 4 cameras"
    );

    let p = &CPOS[index];

    Position::new(
        p.0 * distance,
        p.1 * distance,
        (index as f64) * 90f64.to_radians(),
    )
}

pub fn get_camera_distance_in_square(side_length: f64, fov: f64) -> f64 {
    0.5 * side_length * (
        1. / (
            0.5 * fov
        ).tan() + 1.
    )
}

impl Lerp for Position {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        Position::new(
			    f64::lerp(&s.x, &e.x, t),
			    f64::lerp(&s.y, &e.y, t),
			    f64::lerp(&s.rotation, &e.rotation, t),
		    )
    }
}

pub mod calibration;
pub mod position;
pub mod hosts;

pub trait Lerp {
	fn lerp(s: &Self, e: &Self, t: f64) -> Self;
}

impl Lerp for f64 {
    fn lerp(s: &Self, e: &Self, t: f64) -> Self {
        (1. - t) * s + t * e
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GenerationalValue<T> {
    generation: usize,
    value: T,
}
impl<T> GenerationalValue<T> {
    pub const fn new(value: T) -> Self {
        Self { generation: 0, value }
    }
    pub const fn new_with_generation(value: T, generation: usize) -> Self {
        Self { generation, value }
    }

    pub const fn get(&self) -> &T {
        &self.value
    }
    pub fn set(&mut self, value: T) {
        self.generation += 1;
        self.value = value;
    }
    pub const fn generation(&self) -> usize {
        self.generation
    }
}
impl<T: std::fmt::Display> std::fmt::Display for GenerationalValue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} of {}]", self.value, self.generation)
    }
}

pub fn get_from_stdin<T: std::str::FromStr>(prompt: &str) -> Result<T, &'static str> {
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();
    stdout.write_all(prompt.as_bytes()).map_err(|_| "Failed to write prompt")?;
    stdout.flush().map_err(|_| "Failed to flush stdout")?;
    drop(stdout);

    let mut l = String::new();
    std::io::stdin().read_line(&mut l)
        .map_err(|_| "Couldn't get line")?;

    l.get(..(l.len() - 1))
        .ok_or("Empty")?
        .parse()
        .map_err(|_| "Invalid index")
}

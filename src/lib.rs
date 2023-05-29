use std::{
    fmt::Display,
    time::{Duration, Instant},
};

#[cfg(feature = "cv")]
pub mod cv;
pub mod hosts;
pub mod position;

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
        Self {
            generation: 0,
            value,
        }
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
    stdout
        .write_all(prompt.as_bytes())
        .map_err(|_| "Failed to write prompt")?;
    stdout.flush().map_err(|_| "Failed to flush stdout")?;
    drop(stdout);

    let mut l = String::new();
    std::io::stdin()
        .read_line(&mut l)
        .map_err(|_| "Couldn't get line")?;

    if l.is_empty() {
        return Err("Empty value");
    }

    // exclude newline at the end
    l[..l.len() - 1].parse().map_err(|_| "Invalid value")
}

pub fn yes_no_choice(prompt: &str, default: bool) -> bool {
    let default_text = if default { "Y/n" } else { "y/N" };
    let prompt = &format!("{prompt} ({default_text}) ");

    match get_from_stdin::<String>(prompt) {
        Ok(answer) if !answer.is_empty() => matches!(&answer.to_lowercase()[..], "y" | "yes"),
        _ => default,
    }
}

pub fn choice<T: Display>(
    listed: impl Iterator<Item = (T, bool)>,
    choice_prompt: Option<&str>,
    default_choice: Option<usize>,
) -> Result<usize, &'static str> {
    let mut mapping = vec![];
    for (i, (c, is_choice)) in listed.enumerate() {
        if is_choice {
            print!("{:<3}", mapping.len());
            mapping.push(i);
        } else {
            print!("   ");
        }

        println!("{c}");
    }

    get_from_stdin::<usize>(choice_prompt.unwrap_or("Enter choice: "))
        .and_then(|v| {
            if v >= mapping.len() {
                Err("No such choice")
            } else {
                Ok(mapping[v])
            }
        })
        .or(default_choice.ok_or("No valid default was provided"))
}

#[derive(Debug, Clone, Copy)]
pub struct TimeValidatedValue<T> {
    last_changed: Instant,
    pub valid_time: Duration,
    value: T,
}

impl<T> TimeValidatedValue<T> {
    pub fn new(value: T, valid_time: Duration) -> Self {
        Self {
            last_changed: Instant::now(),
            valid_time,
            value,
        }
    }

    pub const fn new_with_change(value: T, valid_time: Duration, last_changed: Instant) -> Self {
        Self {
            last_changed,
            valid_time,
            value,
        }
    }

    pub fn new_invalid(valid_time: Duration) -> Self
    where
        T: Default,
    {
        Self {
            last_changed: Instant::now() - valid_time,
            value: T::default(),
            valid_time,
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.is_valid() {
            Some(&self.value)
        } else {
            None
        }
    }

    pub fn set(&mut self, value: T) {
        self.last_changed = Instant::now();
        self.value = value;
    }

    pub fn set_with_time(&mut self, value: T, last_changed: Instant) {
        self.last_changed = last_changed;
        self.value = value;
    }

    pub fn is_valid(&self) -> bool {
        self.last_changed.elapsed() <= self.valid_time
    }

    pub const fn last_changed(&self) -> Instant {
        self.last_changed
    }
}

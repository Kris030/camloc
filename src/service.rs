#![allow(non_upper_case_globals)]
use std::{io::Read, thread::{spawn, JoinHandle}, sync::{RwLock, Mutex}, time::{Instant, Duration}, net::{TcpStream, ToSocketAddrs}, f64::NAN, fmt::{Debug, Display}, mem::MaybeUninit};
use crate::{calc::{Coordinates, Setup}, extrapolations::Extrapolation};

static running: RwLock<bool> = RwLock::new(false);

static thread_handle: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static last_known_pos: RwLock<Position> = RwLock::new(Position {
	time: unsafe { std::mem::transmute([0u8; 16]) },
	coordinates: Coordinates::new(NAN, NAN),
});
static extrap: RwLock<Option<Extrapolation>> = RwLock::new(None);

type ServiceSubscriber = fn(Position) -> ();
static subscribtions: RwLock<Vec<ServiceSubscriber>> = RwLock::new(vec![]);

pub fn start<const C: usize>(
	setup: Setup<C>,
	addresses: [impl ToSocketAddrs + Display; C],
	extrapolation: Option<Extrapolation>,
) -> Result<(), String> {
	let mut r = running.write().map_err(|_| "".to_owned())?;
	if *r {
		return Err("Already running".to_owned());
	}
	*r = true;
	drop(r);

	let Ok(mut hg) = thread_handle.lock() else {
		return Err("wut da heell".to_string());
	};

	let Ok(mut ex) = extrap.write() else {
		return Err("xdd".to_string());	
	};
	*ex = extrapolation;
	drop(ex);

	// let connections = addresses.try_map(|a|
	// 	TcpStream::connect(a)
	// ).map_err(|_| "Couldn't connect".to_string())?;
	// FIXME: better ^
	let connections = {
		let mut ind = 0;
		let mut failed = None;
		let connections = addresses
			.map(|a| {
			if failed.is_some() {
				return None;
			}
	
			let ret = if let Ok(c) = TcpStream::connect(a) {
				let _ = c.set_read_timeout(Some(Duration::from_millis(1000)));
				Some(c)
			} else {
				failed = Some(ind);
				None
			};

			ind += 1;

			ret
		});
		if let Some(fi) = failed {
			return Err(format!("Couldn't connect to host #{fi}"));
		}
		connections.map(|c| c.unwrap())
	};

	let handle = spawn(move ||
		run(
			setup,
			connections,
		)
	);
	*hg = Some(handle);

	Ok(())
}

pub fn subscribe(action: fn(Position) -> ()) -> Result<(), String> {
	let Ok(mut sw) = subscribtions.write() else {
		return Err("Couldn't acquire lock for some god awful reason".to_string());
	};

	// let Ok(s) = TcpStream::connect(address) else {
	// 	return Err("Couldn't connect to host".to_string());
	// };

	sw.push(action);
	Ok(())
}

fn run<const C: usize>(
	setup: Setup<C>,
	mut connections: [TcpStream; C],
) {

	// FIXME: just no...
	unsafe { START_TIME.write(Instant::now()); }

	// TODO: figure out how to handle errors
	'outer: loop {
		let Ok(r) = running.read() else {
			break;
		};
		if !*r {
			break;
		}
		drop(r);
		
		let mut pxs = [None; C];
		for i in 0..C {
			let mut buf = [0u8; 8];
			
			if connections[i].read_exact(&mut buf).is_err() {
				break 'outer;
			};

			let px = f64::from_be_bytes(buf);

			if !px.is_nan() {
				pxs[i] = Some(px);
			}
		}

		let Some(pos) = setup.calculate_position(&pxs) else {
			break;
		};

		let calculated_position = Position { coordinates: pos, time: Instant::now() };

		let Ok(mut global_position) = last_known_pos.write() else {
			break;
		};

		*global_position = calculated_position;

		if let Ok(mut ex) = extrap.write() {
			if let Some(ex) = ex.as_mut() {
				ex.extrapolator.add_datapoint(calculated_position);
			}
		}

		let Ok(subs) = subscribtions.read() else {
			break;
		};

		for s in subs.iter() {
			s(calculated_position);
		}
	}
}

pub fn get_position() -> Option<Position> {
	if !*(running.read().ok()?) {
		return None;
	}

	let pos = *last_known_pos.read().ok()?;
	if pos.coordinates.x.is_nan() || pos.coordinates.y.is_nan() {
		return None;
	}

	let now = Instant::now();

	if let Ok(ex) = extrap.read() {
		if let Some(x) = (*ex).as_ref() {
			if now > pos.time + x.invalidate_after {
				return None;
			}

			x.extrapolator.extrapolate(now)
				.map(|extrapolated| Position { coordinates: extrapolated, time: now })
		} else {
			Some(pos)
		}
	} else {
		None
	}
}

pub fn stop() -> Result<(), String> {
	let Ok(mut r) = running.write() else {
		return Err("wut da heeell".to_string());
	};

	if !*r {
		return Err("Not running".to_string());
	}

	*r = false;
	drop(r);

	let Ok(mut handle) = thread_handle.lock() else {
		return Err("wut da heeell".to_string());
	};
	let h = std::mem::replace(&mut *handle, None);
	if let Some(h) = h {
		h.join().map_err(|_| "Couldn't join??".to_string())?;
		Ok(())
	} else {
		Err("No handle?".to_string())
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub coordinates: Coordinates,
    pub time: Instant,
}

static mut START_TIME: MaybeUninit<Instant> = MaybeUninit::uninit();

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{} @ {:?}]", self.coordinates, self.time - unsafe {START_TIME.assume_init()})
    }
}

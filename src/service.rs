#![allow(non_upper_case_globals)]
use std::{io::Read, thread::{spawn, JoinHandle}, sync::{RwLock, Mutex}, time::{Instant, Duration}, net::{TcpStream, ToSocketAddrs}, f64::NAN, fmt::{Debug, Display}};
use crate::calc::{Position, Setup};

static running: RwLock<bool> = RwLock::new(false);

static thread_handle: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static last_known_pos: RwLock<KnownPosition> = RwLock::new(KnownPosition { pos: (NAN, NAN), time: unsafe { std::mem::transmute([0u8; 16]) } });
static extrap: RwLock<Option<Extrapolation>> = RwLock::new(None);

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

	// let mut connections: [TcpStream; C] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
	// for i in 0..C {
	// 	let Ok(sock) = TcpStream::connect(&addresses[i]) else {
	// 		return Err(format!("Couldn't connect to {}", addresses[i]));
	// 	};
	// 	let Ok(_) = sock.set_read_timeout(Some(Duration::from_secs(1))) else {
	// 		return Err("Couldn't set read timeout???".to_string());
	// 	};
	// 	connections[i] = sock;
	// }

	let connections = addresses.try_map(|a|
		TcpStream::connect(a)
	).map_err(|_| "Couldn't connect".to_string())?;

	let handle = spawn(move ||
		run(
			setup,
			connections,
		)
	);
	*hg = Some(handle);

	Ok(())
}

fn run<const C: usize>(
	setup: Setup<C>,
	mut connections: [TcpStream; C],
) {
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
			
			if let Err(_) = connections[i].read_exact(&mut buf) {
				break 'outer;
			};

			let px = f64::from_be_bytes(buf);

			if !px.is_nan() {
				pxs[i] = Some(px);
			}
		}

		let Ok(mut posg) = last_known_pos.write() else {
			break;
		};

		if let Some(pos) = setup.calculate_position(&pxs) {
			let position = KnownPosition { pos, time: Instant::now() };
			*posg = position;

			if let Ok(mut ex) = extrap.write() {
				if let Some(ex) = ex.as_mut() {
					ex.extrapolation_type.add_datapoint(position);
				}
			}
		} else {
			break;
		}
	}
}

pub fn get_position() -> Option<Position> {
	if !*(running.read().ok()?) {
		return None;
	}

	let pos = *last_known_pos.read().ok()?;
	let now = Instant::now();

	if let Ok(ex) = extrap.read() {
		if let Some(x) = (*ex).as_ref() {
			if now > pos.time + x.invalidate_after {
				return None;
			}

			Some(x.extrapolation_type.extrapolate(now))
		} else {
			Some(pos.pos)
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
pub struct KnownPosition {
    pos: Position,
    time: Instant,
}

pub trait Extrapolator {
    fn add_datapoint(&mut self, position: KnownPosition);
    fn extrapolate(&self, time: Instant) -> Position;
}

pub struct Extrapolation {
    pub extrapolation_type: Box<dyn Extrapolator + Send + Sync>,
    pub invalidate_after: Duration,
}

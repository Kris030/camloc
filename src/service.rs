#![allow(non_upper_case_globals)]
#![allow(unused)]

use std::{io::Read, thread::{spawn, JoinHandle}, sync::{RwLock, Mutex, mpsc::{channel, Receiver}}, time::{Instant, Duration}, net::TcpStream, borrow::BorrowMut, f64::NAN};

use crate::calc::{Position, Setup};

static running: RwLock<bool> = RwLock::new(false);

static thread_handle: Mutex<JoinHandle<()>> = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
static last_known_pos: RwLock<KnownPosition> = RwLock::new(KnownPosition { pos: (NAN, NAN), time: unsafe { std::mem::transmute([0u8; 16]) } });
static extrap: RwLock<Option<Extrapolation>> = RwLock::new(None);

pub fn start<const C: usize>(
	setup: Setup<C>,
	addresses: [String; C],
	extrapolation: Option<Extrapolation>,
) -> Result<(), String> {
	let mut r = running.write().map_err(|_| "".to_owned())?;
	if *r {
		return Err("Already running".to_owned());
	}
	*r = true;

	let handle = spawn(move || run(
		setup,
		addresses,
		extrapolation,
	));

	Ok(())
}

async fn run<const C: usize>(
	setup: Setup<C>,
	addresses: [String; C],
	extrapolation: Option<Extrapolation>,
) {
	let mut connections: [TcpStream; C] = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
	for i in 0..C {
		let Ok(sock) = TcpStream::connect(&addresses[i]) else {
			return;
		};
		connections[i] = sock;
	}
	
	loop {
		let Ok(r) = running.read() else {
			break;
		};
		if !*r {
			break;
		}
		
		let Ok(mut posg) = last_known_pos.write() else {
			break;
		};

		let mut pxs = [None; C];
		for i in 0..C {
			let mut buf = [0u8; 8];
			connections[i].read(&mut buf);
			let px = f64::from_le_bytes(buf);

			if !px.is_nan() {
				pxs[i] = Some(px);
			}
		}

		if let Some(pos) = setup.calculate_position(&pxs) {
			*posg = KnownPosition { pos, time: Instant::now() };
		}
	}
}

pub fn get_position() -> Option<Position> {
	if !*running.read().ok()? {
		return None;
	}

	let pos = *last_known_pos.read().ok()?;
	let now = Instant::now();

	if let Ok(ex) = extrap.read() {
		let x = (*ex).as_ref()?;
		if now > pos.time + x.invalidate_after {
			return None;
		}

		Some(x.extrapolation_type.extrapolate(now))
	} else {
		Some(pos.pos)
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

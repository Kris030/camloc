#![allow(unused)]

use std::{io::Read, thread::JoinHandle, sync::{RwLock, Mutex}, time::{Instant, Duration}, net::TcpStream};
use crate::calc::{Position, Setup};

static mut RUNNING: RwLock<bool> = RwLock::new(false);

struct ServiceState {
	thread_handle: Mutex<JoinHandle<()>>,
	last_known_pos: RwLock<KnownPosition>,
}
pub struct Service<const C: usize> {
	state: Option<ServiceState>,

    extrapolation: Option<Extrapolation>,
	
	setup: Setup<C>,
	hosts: Vec<TcpStream>,
}

impl<const C: usize> Service<C> {
	pub fn start(setup: Setup<C>, addresses: [String; C], extrapolation: Option<Extrapolation>) -> Result<Service<C>, std::io::Error> {
		let mut s = Service {
    		extrapolation,
			hosts: vec![],
			state: None,
			setup,
		};

		for c_addr in addresses {
			s.hosts.push(TcpStream::connect(c_addr)?);
		}

		// let handle = std::thread::spawn(|| );

		Ok(s)
	}

	pub fn get_position(&self) -> Option<Position> {
		
		Some((0., 0.))
	}
}

pub struct KnownPosition {
    pos: Position,
    time: Instant,
}

pub trait Extrapolator {
    fn add_datapoint(&mut self, position: KnownPosition);
    fn extrapolate(&self) -> Position;
}

pub struct Extrapolation {
    pub extrapolation_type: Box<dyn Extrapolator + Send + Sync>,
    pub invalidate_after: Duration,
}

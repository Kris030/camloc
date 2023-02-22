#![allow(non_upper_case_globals)]
use tokio::{net::TcpStream, spawn, task::{JoinSet, JoinHandle}, sync::{RwLock, Mutex}, io::AsyncReadExt};
use std::{sync::Arc, time::{Instant, Duration}, f64::NAN, fmt::{Debug, Display}};

use crate::{calc::{Coordinates, Setup}, extrapolations::Extrapolation, scanning::AddressTemplate};

type ServiceSubscriber = fn(Position) -> ();
pub struct LocationService {
	running: RwLock<bool>,
	last_known_pos: RwLock<Position>,
	extrap: RwLock<Option<Extrapolation>>,
	subscribtions: RwLock<Vec<ServiceSubscriber>>,
	start_time: RwLock<Instant>,
	setup: RwLock<Setup>,
	connections: Mutex<Vec<(String, TcpStream)>>,
}

pub struct LocationServiceHandle {
	handle: Option<JoinHandle<()>>,
	service: Arc<LocationService>,
}

impl Drop for LocationServiceHandle {
    fn drop(&mut self) {
		let handle = std::mem::replace(&mut self.handle, None)
			.expect("Handle should always be Some");

		let r = self.service.running.write();
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async {
				let mut r = r.await;
				*r = false;
				drop(r);
				handle.await
			})
		}).expect("Should always be able join the task");
	}
}

impl LocationService {
	pub async fn start_scanning(
		setup: Setup,
		address_generator: AddressTemplate,
		extrapolation: Option<Extrapolation>,
	) -> Result<LocationServiceHandle, String> {
		use tokio::time::sleep;

		let start_time = Instant::now();
		let initial_connections: Vec<(String, TcpStream)> = async {
			let mut js = JoinSet::new();
			for a in address_generator.into_iter() {
				js.spawn(async move {
					if let Ok(c) = TcpStream::connect(&a).await {
						Some((a, c))
					} else {
						None
					}
				});
			}

			let mut cons = vec![];
			while let Some(v) = js.join_next().await {
				let Ok(v) = v else {
					return Err("Connect task?? failed???".to_string())
				};
				let Some(v) = v else {
					return Err("Failed to connect to to one of the hosts".to_string())
				};

				cons.push(v);
			}

			Ok(cons)
		}.await?;

		let instance = LocationService {
			running: RwLock::new(true),
			last_known_pos: RwLock::new(Position {
				start_time: unsafe { std::mem::transmute([0u8; 16]) },
				time: unsafe { std::mem::transmute([0u8; 16]) },
				coordinates: Coordinates::new(NAN, NAN),
				interpolated: None,
			}),
			extrap: RwLock::new(extrapolation),
			subscribtions: RwLock::new(vec![]),
			start_time: RwLock::new(start_time),
			connections: Mutex::new(initial_connections),
			setup: RwLock::new(setup),
		};

		let arc = Arc::new(instance);
		let ret = arc.clone();
		let sweep_handle = arc.clone();

		let handle = spawn(
			Self::run(
				arc,
				start_time,
			)
		);

		spawn(async move {
			loop {
				let r = sweep_handle.running.read().await;
				if !*r { break; }
				drop(r);

				for a in address_generator.into_iter() {
					let mut conn_handle = sweep_handle.connections.lock().await;
					if conn_handle.iter().any(|(addr, _)| *addr == a) {
						continue;
					}

					if let Ok(new_connection) = TcpStream::connect(&a).await {
						// FIXME: add camera info to setup
						conn_handle.push((a, new_connection));
					}
					drop(conn_handle);

					sleep(Duration::from_millis(50)).await;
				}
				sleep(Duration::from_millis(500)).await;
			}
		});

		Ok(LocationServiceHandle {
			handle: Some(handle),
			service: ret,
		})
	}

	pub async fn start(
		setup: Setup,
		addresses: impl IntoIterator<Item = String>,
		extrapolation: Option<Extrapolation>,
	) -> Result<LocationServiceHandle, String> {
		let connections = async {
			let mut js = JoinSet::new();
			for a in addresses {
				js.spawn(async {
					if let Ok(s) = TcpStream::connect(&a).await {
						Some((a, s))
					} else {
						None
					}
				});
			}

			let mut cons = vec![];
			while let Some(v) = js.join_next().await {
				match v {
					Ok(Some(v)) => cons.push(v),
					_ => return Err("Couldn't connect to all hosts".to_string()),
				}
			}

			Ok(cons)
		}.await?;

		let start_time = Instant::now();
	
		let instance = LocationService {
			running: RwLock::new(true),
			last_known_pos: RwLock::new(Position {
				start_time: unsafe { std::mem::transmute([0u8; 16]) },
				time: unsafe { std::mem::transmute([0u8; 16]) },
				coordinates: Coordinates::new(NAN, NAN),
				interpolated: None,
			}),
			extrap: RwLock::new(extrapolation),
			subscribtions: RwLock::new(vec![]),
			start_time: RwLock::new(start_time),
			connections: Mutex::new(connections),
			setup: RwLock::new(setup),
		};

		let arc = Arc::new(instance);
		let ret = arc.clone();

		let handle = spawn(
			Self::run(
				arc,
				start_time,
			)
		);

		Ok(LocationServiceHandle {
			handle: Some(handle),
			service: ret,
		})
	}

	async fn run(
		self: Arc<LocationService>,
		start_time: Instant,
	) {

		// FIXME: figure out how to handle errors
		'outer: loop {
			let r = self.running.read().await;
			if !*r {
				break;
			}
			drop(r);

			let mut pxs = vec![None; self.setup.write().await.cameras.len()];
			for (i, c) in self.connections.lock().await.iter_mut().enumerate() {
				let mut buf = [0u8; 8];
				if c.1.read_exact(&mut buf).await.is_err() {
					break 'outer;
				}

				pxs[i] = Some(f64::from_be_bytes(buf));
			}

			let Some(pos) = self.setup.read().await.calculate_position(pxs) else {
				break;
			};

			let calculated_position = Position {
				coordinates: pos,
				start_time,
				time: Instant::now(),
				interpolated: None,
			};

			let mut global_position = self.last_known_pos.write().await;
			*global_position = calculated_position;

			let mut ex = self.extrap.write().await;
			let Some(ref mut ex) = *ex else {
				break;
			};
			ex.extrapolator.add_datapoint(calculated_position);
	
			let subs = self.subscribtions.read().await;
	
			for s in subs.iter() {
				s(calculated_position);
			}
		}
	}

}

impl LocationServiceHandle {

	pub async fn subscribe(&self, action: fn(Position) -> ()) -> Result<(), String> {
		let mut sw = self.service.subscribtions.write().await;
		sw.push(action);
		Ok(())
	}

	pub async fn get_position(&self) -> Option<Position> {
		if !*(self.service.running.read().await) {
			return None;
		}
		
		let pos = self.service.last_known_pos.read().await;
		if pos.coordinates.x.is_nan() || pos.coordinates.y.is_nan() {
			return None;
		}
		
		let start_time = self.service.start_time.read().await;
		let now = Instant::now();
	
		let ex = self.service.extrap.read().await;
		if let Some(x) = (*ex).as_ref() {
			if now > pos.time + x.invalidate_after {
				return None;
			}

			x.extrapolator.extrapolate(now)
				.map(|extrapolated| Position {
					coordinates:
					extrapolated,
					start_time: *start_time,
					time: now,
					interpolated: x.extrapolator
						.get_last_datapoint()
						.map(|p| now - p.time),
				})
		} else {
			Some(*pos)
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub coordinates: Coordinates,
    start_time: Instant,
    pub time: Instant,

	/// - None - not interpolated
	/// - Some(d) - interpolated by d time
	pub interpolated: Option<Duration>,
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let coords = &self.coordinates;
		let t = self.time - self.start_time;

		if let Some(from) = self.interpolated {
			write!(f, "[{coords} @ {from:.2?} -> {t:.2?}]")
		} else {
			write!(f, "[{coords} @ {t:.2?}]")
		}
    }
}

#![allow(non_upper_case_globals)]
use tokio::{net::{TcpStream, ToSocketAddrs}, spawn, task::{JoinSet, JoinHandle}, io::AsyncReadExt, sync::RwLock};
use std::{sync::Arc, time::{Instant, Duration}, f64::NAN, fmt::{Debug, Display}};

use crate::{calc::{Coordinates, Setup}, extrapolations::Extrapolation};

type ServiceSubscriber = fn(Position) -> ();
pub struct LocationService<const C: usize> {
	running: RwLock<bool>,
	last_known_pos: RwLock<Position>,
	extrap: RwLock<Option<Extrapolation>>,
	subscribtions: RwLock<Vec<ServiceSubscriber>>,
	start_time: RwLock<Instant>,
}

pub struct LocationServiceHandle<const C: usize> {
	handle: Option<JoinHandle<()>>,
	service: Arc<LocationService<C>>,
}

impl<const C: usize> Drop for LocationServiceHandle<C> {
    fn drop(&mut self) {
		let handle = std::mem::replace(&mut self.handle, None)
			.expect("Handle should always be Some");
		
		let r = self.service.running.write();
		
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async {
				let mut r = r.await;
				*r = false;

				handle.await
			})
		}).expect("Should always be able join the task");
		// TODO: wait for joinhandle
    }
}

impl<const C: usize> LocationService<C> {
	pub async fn start(
		setup: Setup<C>,
		addresses: [impl ToSocketAddrs + Copy + Send + 'static; C],
		extrapolation: Option<Extrapolation>,
	) -> Result<LocationServiceHandle<C>, String> {
		let connections = async {
			let mut js = JoinSet::new();
			for a in addresses {
				js.spawn(TcpStream::connect(a));
			}

			let mut cons = vec![];
			while let Some(v) = js.join_next().await {
				match v {
					Ok(Ok(s)) => cons.push(s),
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
		};

		let arc = Arc::new(instance);
		let ret = arc.clone();

		let handle = spawn(
			Self::run(
				arc,
				setup,
				connections,
				start_time,
			)
		);

		Ok(LocationServiceHandle {
			handle: Some(handle),
			service: ret,
		})
	}

	async fn run(
		self: Arc<LocationService<C>>,
		setup: Setup<C>,
		mut connections: Vec<TcpStream>,
		start_time: Instant,
	) {
	
		// TODO: figure out how to handle errors
		'outer: loop {
			let r = self.running.read().await;
			if !*r {
				break;
			}
			drop(r);

			// let tasks: Vec<_> = connections.iter_mut().map(|c| async {
			// 	use tokio::io::AsyncReadExt;
			// 	let mut buf = [0u8; 8];
			// 	if c.read_exact(&mut buf).await.is_ok() {
			// 		Some(buf)
			// 	} else {
			// 		None
			// 	}
			// }).collect();
			
			// let mut js = JoinSet::new();
			// for t in tasks {
			// 	js.spawn(t);
			// }

			let mut pxs = [None; C];
			{
				for (i, c) in connections.iter_mut().enumerate() {
					// 	use tokio::io::AsyncReadExt;
					let mut buf = [0u8; 8];
					if c.read_exact(&mut buf).await.is_err() {
						break 'outer;
					}

					pxs[i] = Some(f64::from_be_bytes(buf));
				}
			}

			let Some(pos) = setup.calculate_position(&pxs) else {
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

impl<const C: usize> LocationServiceHandle<C> {

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

	// pub fn stop(self) -> Result<(), String> {
	// 	let Ok(mut r) = self.running.write() else {
	// 		return Err("wut da heeell".to_string());
	// 	};
	
	// 	if !*r {
	// 		return Err("Not running".to_string());
	// 	}
	
	// 	*r = false;
	// 	drop(r);
	
	// 	let Ok(mut handle) = self.thread_handle.lock() else {
	// 		return Err("wut da heeell".to_string());
	// 	};
	// 	let h = std::mem::replace(&mut *handle, None);
	// 	if let Some(h) = h {
	// 		h.join().await.map_err(|_| "Couldn't join??".to_string())?;
	// 		Ok(())
	// 	} else {
	// 		Err("No handle?".to_string())
	// 	}
	// }
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

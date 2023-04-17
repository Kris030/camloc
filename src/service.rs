use std::{sync::Arc, time::{Instant, Duration}, f64::NAN, fmt::{Debug, Display}, mem, net::SocketAddr};
use tokio::{net::UdpSocket, spawn, task::{JoinHandle}, sync::{RwLock, Mutex}};

use crate::{calc::{Coordinate, Setup, PlacedCamera, CameraInfo}, extrapolations::Extrapolation, utils::GenerationalValue};

static DATA_VALIDITY: Duration = Duration::from_millis(500);

struct ClientInfo {
	last_value: GenerationalValue<(f64, Instant)>,
	address: SocketAddr,
}
impl ClientInfo {
	fn new(address: SocketAddr, last_value: GenerationalValue<(f64, Instant)>) -> Self {
		Self { address, last_value, }
	}
}

type ConnectionSubscriber = fn(SocketAddr, PlacedCamera) -> ();
type ServiceSubscriber = fn(Position) -> ();
pub struct LocationService {
	connection_subscriptions: RwLock<Vec<ConnectionSubscriber>>,
	subscriptions: RwLock<Vec<ServiceSubscriber>>,
	extrap: RwLock<Option<Extrapolation>>,
	last_known_pos: RwLock<Position>,
	clients: Mutex<Vec<ClientInfo>>,
	start_time: RwLock<Instant>,
	running: RwLock<bool>,
	setup: RwLock<Setup>,
}

pub struct LocationServiceHandle {
	handle: Option<JoinHandle<Result<(), String>>>,
	service: Arc<LocationService>,
}

impl Drop for LocationServiceHandle {
    fn drop(&mut self) {
		let handle = mem::replace(&mut self.handle, None)
			.expect("Handle should always be Some");

		let r = self.service.running.write();
		tokio::task::block_in_place(|| {
			tokio::runtime::Handle::current().block_on(async {
				let mut r = r.await;
				*r = false;
				drop(r);
				handle.await
			})
		}).expect("Should always be able join the task").unwrap();
	}
}

impl LocationService {
	pub async fn start(
		extrapolation: Option<Extrapolation>,
		port: u16
	) -> Result<LocationServiceHandle, String> {
		let start_time = Instant::now();

		let udp_socket = UdpSocket::bind(("127.0.0.1", port)).await
			.map_err(|_| "Couldn't create socket")?;

		let instance = LocationService {
			last_known_pos: RwLock::new(Position::default()),
			setup: RwLock::new(Setup::new_freehand(vec![])),
			connection_subscriptions: RwLock::new(vec![]),
			start_time: RwLock::new(start_time),
			subscriptions: RwLock::new(vec![]),
			extrap: RwLock::new(extrapolation),
			clients: Mutex::new(vec![]),
			running: RwLock::new(true),
		};

		let arc = Arc::new(instance);
		let ret = arc.clone();

		let handle = spawn(
			Self::run(
				arc,
				udp_socket,
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
		udp_socket: UdpSocket,
		start_time: Instant,
	) -> Result<(), String> {
		let mut min_generation = 0;
		let mut buf = [0u8; 64];

		while *self.running.read().await {
			let (recv_len, address) = udp_socket.recv_from(&mut buf).await
				.map_err(|_| "Error while recieving")?;
			let time = Instant::now();

			match recv_len {

				// "organizer bonk"
				1 if buf[0] == 0x0b => {
					udp_socket.send_to(&[0x5a], address).await
						.map_err(|_| "Error while sending")?;
				},

				// update value
				8 => {
					let mut clients = self.clients.lock().await;
					let mut ci = None;
					let (mut mins, mut mini) = (0, 0);

					let mut values = vec![None; clients.len()];
					for (i, c) in clients.iter().enumerate() {
						let (value, time) = *c.last_value.get();
						values[i] = if time.elapsed() > DATA_VALIDITY {
							None
						} else {
							Some(value)
						};

						if c.last_value.generation() == min_generation {
							mins += 1;
							mini = i;
						}
						if c.address == address {
							ci = Some(i);
						}
					}
					if let Some(ci) = ci {
						clients[ci].last_value.set((
							f64::from_be_bytes(buf[..8].try_into().unwrap()),
							time
						));

						if mins == 1 && mini == ci {
							min_generation += 1;
							self.update_position(start_time, &values).await?;
						}
					}
				},

				// connection request
				33 if buf[0] == 0xcc => {
					let x = f64::from_be_bytes(buf[1..9].try_into().unwrap());
					let y = f64::from_be_bytes(buf[9..17].try_into().unwrap());
					let r = f64::from_be_bytes(buf[17..25].try_into().unwrap());
					let f = f64::from_be_bytes(buf[25..33].try_into().unwrap()); // TODO: ?

					self.clients.lock().await.push(ClientInfo::new(
						address,
						GenerationalValue::new_with_generation(
							(NAN, start_time),
							min_generation
						),
					));

					let cam = PlacedCamera::new(
							CameraInfo::new(f),
							Coordinate::new(x, y),
							r
					);
					self.setup.write().await.cameras.push(cam);

					for s in self.connection_subscriptions.read().await.iter() {
						s(address, cam);
					}
				},

				_ => return Err("Recieved invalid number of bytes".to_string()),
			}
		}

		Ok(())
	}

	async fn update_position(
		self: &Arc<LocationService>,
		start_time: Instant,
		pxs: &Vec<Option<f64>>
	) -> Result<(), String> {
		let Some(pos) = self.setup.read().await.calculate_position(pxs) else { return Ok(()); };

		let calculated_position = Position {
			coordinates: pos,
			start_time,
			time: Instant::now(),
			interpolated: None,
		};

		let mut global_position = self.last_known_pos.write().await;
		*global_position = calculated_position;

		let mut ex = self.extrap.write().await;
		if let Some(ref mut ex) = *ex {
			ex.extrapolator.add_datapoint(calculated_position);
		};

		let subs = self.subscriptions.read().await;
		for s in subs.iter() {
			s(calculated_position);
		}

		Ok(())
	}
}

impl LocationServiceHandle {
	
	pub async fn subscribe_connection(&self, action: ConnectionSubscriber) {
		let mut sw = self.service.connection_subscriptions.write().await;
		sw.push(action);
	}

	pub async fn subscribe(&self, action: ServiceSubscriber) {
		let mut sw = self.service.subscriptions.write().await;
		sw.push(action);
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
    pub coordinates: Coordinate,
    start_time: Instant,
    pub time: Instant,

	/// - None - not interpolated
	/// - Some(d) - interpolated by d time
	pub interpolated: Option<Duration>,
}

impl Default for Position {
    fn default() -> Self {
        Self {
			start_time: unsafe { mem::transmute([0u8; 16]) },
			time: unsafe { mem::transmute([0u8; 16]) },
			coordinates: Coordinate::new(NAN, NAN),
			interpolated: None,
		}
    }
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

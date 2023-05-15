use camloc_common::{
    hosts::{Command, HostInfo, HostState, HostType},
    position::Position,
    GenerationalValue,
};
use std::{
    f64::NAN,
    fmt::{Debug, Display},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::UdpSocket,
    spawn,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    calc::{PlacedCamera, Setup},
    extrapolations::Extrapolation,
};

static DATA_VALIDITY: Duration = Duration::from_millis(500);

struct ClientInfo {
    last_value: GenerationalValue<(f64, Instant)>,
    address: SocketAddr,
}
impl ClientInfo {
    fn new(address: SocketAddr, last_value: GenerationalValue<(f64, Instant)>) -> Self {
        Self {
            address,
            last_value,
        }
    }
}

type SubscriberFnRet = Pin<Box<dyn Future<Output = ()> + Send>>;

pub enum Subscriber {
    Connection(fn(SocketAddr, PlacedCamera) -> SubscriberFnRet),
    Position(fn(TimedPosition) -> SubscriberFnRet),
}

pub struct LocationService {
    subscriptions: RwLock<Vec<Subscriber>>,
    extrap: RwLock<Option<Extrapolation>>,
    last_known_pos: RwLock<TimedPosition>,
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
        let handle = self.handle.take().expect("Handle should always be Some");

        let r = self.service.running.write();
        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut r = r.await;
                *r = false;
                drop(r);

                handle.await
            })
        });

        res.expect("Should always be able join the task").unwrap();
    }
}

impl LocationService {
    pub async fn start(
        extrapolation: Option<Extrapolation>,
        port: u16,
    ) -> Result<LocationServiceHandle, String> {
        let start_time = Instant::now();

        let udp_socket = UdpSocket::bind(("0.0.0.0", port))
            .await
            .map_err(|_| "Couldn't create socket")?;

        let instance = LocationService {
            last_known_pos: TimedPosition {
                start_time,
                time: start_time,
                position: Position::new(NAN, NAN, NAN),
                interpolated: None,
            }
            .into(),
            setup: Setup::new_freehand(vec![]).into(),
            subscriptions: vec![].into(),
            start_time: start_time.into(),
            extrap: extrapolation.into(),
            clients: vec![].into(),
            running: true.into(),
        };

        let arc = Arc::new(instance);
        let ret = arc.clone();

        let handle = spawn(Self::run(arc, udp_socket, start_time));

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

        let (cube, organizer) = loop {
            let r = self.running.read().await;
            if !*r {
                return Ok(());
            }
            drop(r);

            let Ok((len, addr)) = udp_socket.recv_from(&mut buf).await else {
                continue;
            };
            match buf[..len].try_into() {
                Ok(Command::StartServer { cube }) => break (cube, addr),
                Ok(Command::Ping) => {
                    udp_socket
                        .send_to(
                            &[HostInfo {
                                host_type: HostType::Server,
                                host_state: HostState::Idle,
                            }
                            .try_into()
                            .unwrap()],
                            addr,
                        )
                        .await
                        .map_err(|_| "Error while sending")?;
                }
                _ => (),
            }
        };

        loop {
            let r = self.running.read().await;
            if !*r {
                break;
            }
            drop(r);

            let Ok(recv_result) = tokio::time::timeout(
				Duration::from_secs(1),
				udp_socket.recv_from(&mut buf)
			).await else {
				continue;
			};
            let (recv_len, recv_addr) = recv_result.map_err(|_| "Error while recieving")?;

            let recv_time = Instant::now();

            match buf[..recv_len].try_into() {
                // "organizer bonk"
                Ok(Command::Ping) => {
                    udp_socket
                        .send_to(
                            &[HostInfo {
                                host_type: HostType::Server,
                                host_state: HostState::Running,
                            }
                            .try_into()
                            .unwrap()],
                            recv_addr,
                        )
                        .await
                        .map_err(|_| "Error while sending")?;
                }

                // update value
                Ok(Command::ValueUpdate {
                    marker_id,
                    value,
                    rotation,
                }) => {
                    let mut clients = self.clients.lock().await;
                    let mut ci = None;
                    let (mut mins, mut mini) = (0, 0);

                    let mut values = vec![None; clients.len()];
                    for (i, c) in clients.iter().enumerate() {
                        let (value, time) = *c.last_value.get();
                        values[i] = if recv_time - time <= DATA_VALIDITY {
                            Some(value)
                        } else {
                            None
                        };

                        if c.last_value.generation() == min_generation {
                            mins += 1;
                            mini = i;
                        }
                        if c.address == recv_addr {
                            ci = Some(i);
                        }
                    }
                    if let Some(ci) = ci {
                        clients[ci].last_value.set((value, recv_time));

                        if mins == 1 && mini == ci {
                            min_generation += 1;
                            self.update_position(start_time, &values).await?;
                        }
                    }
                }

                // connection request
                Ok(Command::Connect { position, fov }) => {
                    self.clients.lock().await.push(ClientInfo::new(
                        recv_addr,
                        GenerationalValue::new_with_generation((NAN, start_time), min_generation),
                    ));

                    let cam = PlacedCamera::new(position, fov);
                    self.setup.write().await.cameras.push(cam);

                    for s in self.subscriptions.read().await.iter() {
                        if let Subscriber::Connection(s) = s {
                            s(recv_addr, cam).await;
                        }
                    }
                }

                // TODO: update value
                Ok(Command::InfoUpdate { .. }) => todo!(),

                _ => return Err("Recieved invalid number of bytes".to_string()),
            }
        }

        for c in self.clients.lock().await.iter() {
            udp_socket
                .send_to(&[Command::STOP], c.address)
                .await
                .map_err(|_| "Couldn't tell all clients to stop")?;
        }

        println!("Server shut down");

        Ok(())
    }

    async fn update_position(
        self: &Arc<LocationService>,
        start_time: Instant,
        pxs: &Vec<Option<f64>>,
    ) -> Result<(), String> {
        let Some(position) = self.setup.read().await.calculate_position(pxs) else { return Ok(()); };

        let calculated_position = TimedPosition {
            position,
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
            if let Subscriber::Position(s) = s {
                s(calculated_position).await;
            }
        }

        Ok(())
    }
}

impl LocationServiceHandle {
    pub async fn subscribe(&self, action: Subscriber) {
        let mut sw = self.service.subscriptions.write().await;
        sw.push(action);
    }

    pub async fn get_position(&self) -> Option<TimedPosition> {
        if !*(self.service.running.read().await) {
            return None;
        }

        let pos = self.service.last_known_pos.read().await;
        if pos.position.x.is_nan() || pos.position.y.is_nan() {
            return None;
        }

        let start_time = self.service.start_time.read().await;
        let now = Instant::now();

        let ex = self.service.extrap.read().await;
        if let Some(x) = &*ex {
            if now > pos.time + x.invalidate_after {
                return None;
            }

            x.extrapolator
                .extrapolate(now)
                .map(|extrapolated| TimedPosition {
                    position: extrapolated,
                    start_time: *start_time,
                    time: now,
                    interpolated: x.extrapolator.get_last_datapoint().map(|p| now - p.time),
                })
        } else {
            Some(*pos)
        }
    }

    pub async fn stop(self) {
        drop(self)
    }
    pub async fn is_running(&self) -> bool {
        *self.service.running.read().await
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimedPosition {
    pub position: Position,
    start_time: Instant,
    pub time: Instant,

    /// - None - not interpolated
    /// - Some(d) - interpolated by d time
    pub interpolated: Option<Duration>,
}

impl Display for TimedPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pos = &self.position;
        let t = self.time - self.start_time;

        if let Some(from) = self.interpolated {
            write!(f, "[{pos} @ {from:.2?} -> {t:.2?}]")
        } else {
            write!(f, "[{pos} @ {t:.2?}]")
        }
    }
}

use camloc_common::{
    hosts::{ClientData, Command, HostInfo, HostState, HostType},
    position::Position,
    TimeValidatedValue,
};
use futures::future::try_join_all;
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
    calc::{MotionData, MotionHint, PlacedCamera, PositionData, Setup},
    extrapolations::{Extrapolation, Extrapolator},
};

static DATA_VALIDITY: Duration = Duration::from_millis(500);

struct ClientInfo {
    last_data: TimeValidatedValue<ClientData>,
    camera: PlacedCamera,
    address: SocketAddr,
}
impl ClientInfo {
    fn new(
        address: SocketAddr,
        camera: PlacedCamera,
        last_data: TimeValidatedValue<ClientData>,
    ) -> Self {
        Self {
            address,
            camera,
            last_data,
        }
    }
}

type RetFuture<T = Result<(), &'static str>> = Pin<Box<dyn Future<Output = T> + Send>>;

pub enum Subscriber {
    Connection(fn(SocketAddr, PlacedCamera) -> RetFuture),
    Disconnection(fn(SocketAddr, PlacedCamera) -> RetFuture),
    Position(fn(TimedPosition) -> RetFuture),
}

/// TODO: revert to dyn because runtime changeing?
pub struct LocationService<
    E: Send + Extrapolator,
    C: FnMut() -> F,
    F: Future<Output = Option<f64>> + Send,
> {
    motion_data: RwLock<Option<MotionData>>,
    subscriptions: RwLock<Vec<Subscriber>>,
    extrap: RwLock<Option<Extrapolation<E>>>,
    last_known_pos: RwLock<TimedPosition>,
    compass: RwLock<Option<C>>,
    clients: Mutex<Vec<ClientInfo>>,
    start_time: RwLock<Instant>,
    running: RwLock<bool>,
}

pub struct LocationServiceHandle<
    E: Send + Extrapolator,
    C: FnMut() -> F,
    F: Future<Output = Option<f64>> + Send,
> {
    handle: Option<JoinHandle<Result<(), String>>>,
    service: Arc<LocationService<E, C, F>>,
}

impl<E: Send + Extrapolator, C: FnMut() -> F, F: Future<Output = Option<f64>> + Send> Drop
    for LocationServiceHandle<E, C, F>
{
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

impl<
        E: Send + Sync + Extrapolator + 'static,
        C: 'static + Send + Sync + FnMut() -> F,
        F: 'static + Future<Output = Option<f64>> + Send + Sync,
    > LocationService<E, C, F>
{
    pub async fn start(
        extrapolation: Option<Extrapolation<E>>,
        port: u16,
        compass: Option<C>,
    ) -> Result<LocationServiceHandle<E, C, F>, &'static str> {
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
            subscriptions: vec![].into(),
            start_time: start_time.into(),
            extrap: extrapolation.into(),
            compass: compass.into(),
            motion_data: None.into(),
            clients: vec![].into(),
            running: true.into(),
        };

        let arc = Arc::new(instance);
        let ret = arc.clone();

        let handle = spawn(arc.run(udp_socket, start_time));

        Ok(LocationServiceHandle {
            handle: Some(handle),
            service: ret,
        })
    }

    async fn run(
        self: Arc<LocationService<E, C, F>>,
        udp_socket: UdpSocket,
        start_time: Instant,
    ) -> Result<(), String> {
        let mut buf = [0u8; 64];

        let (cube, _organizer) = loop {
            let r = self.running.read().await;
            if !*r {
                return Ok(());
            }
            drop(r);

            let Ok(recv_result) = tokio::time::timeout(
				Duration::from_secs(1),
				udp_socket.recv_from(&mut buf)
			).await else {
				continue;
			};
            let (len, addr) = recv_result.map_err(|_| "Error while recieving")?;

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
                Ok(Command::ValueUpdate(ClientData {
                    marker_id,
                    target_x_position: value,
                })) => {
                    // TODO: clean up
                    let mut clients = self.clients.lock().await;
                    let mut client_index = None;
                    let (mut min_t, mut min_index) = (start_time, 0);

                    let mut values = vec![None; clients.len()];
                    for (i, c) in clients.iter().enumerate() {
                        values[i] = if let Some(data) = c.last_data.get() {
                            if min_t < c.last_data.last_changed() {
                                min_t = c.last_data.last_changed();
                                min_index = i;
                            }
                            Some(*data)
                        } else {
                            None
                        };
                        if c.address == recv_addr {
                            client_index = Some(i);
                        }
                    }
                    if let Some(ci) = client_index {
                        clients[ci]
                            .last_data
                            .set_with_time(ClientData::new(marker_id, value), recv_time);

                        if min_index == ci {
                            self.update_position(start_time, &values[..], cube).await?;
                        }
                    }
                }

                // connection request
                Ok(Command::Connect { position, fov }) => {
                    // TODO: do it better?
                    let camera = PlacedCamera::new(position, fov);
                    self.clients.lock().await.push(ClientInfo::new(
                        recv_addr,
                        camera,
                        TimeValidatedValue::new_with_change(
                            ClientData::new(255, NAN),
                            DATA_VALIDITY,
                            recv_time - DATA_VALIDITY,
                        ),
                    ));

                    try_join_all(self.subscriptions.read().await.iter().filter_map(|s| {
                        if let Subscriber::Connection(s) = s {
                            Some(s(recv_addr, camera))
                        } else {
                            None
                        }
                    }))
                    .await?;
                }

                Ok(Command::InfoUpdate {
                    client_ip,
                    position,
                    fov,
                }) => {
                    let mut clients = self.clients.lock().await;
                    for c in clients.iter_mut() {
                        if c.address.ip().to_string() == client_ip {
                            c.camera.position = position;
                            if let Some(fov) = fov {
                                c.camera.fov = fov;
                            }
                            break;
                        }
                    }
                }

                Ok(Command::Stop) => break,

                Ok(Command::Disconnect) => {
                    let mut clients = self.clients.lock().await;
                    for i in 0..clients.len() {
                        if clients[i].address == recv_addr {
                            clients.remove(i);
                            break;
                        }
                    }
                }

                _ => (),
            }
        }

        try_join_all(self.clients.lock().await.iter().map(|c| async {
            udp_socket
                .send_to(&[Command::STOP], c.address)
                .await
                .map_err(|_| "Couldn't tell all clients to stop")?;
            Ok::<(), &'static str>(())
        }))
        .await?;

        Ok(())
    }

    async fn update_position(
        self: &Arc<LocationService<E, C, F>>,
        start_time: Instant,
        pxs: &[Option<ClientData>],
        cube: [u8; 4],
    ) -> Result<(), String> {
        let motion_data = self.motion_data.read().await;

        let mut compass = self.compass.write().await;
        let compass_value = if let Some(compass) = &mut *compass {
            compass().await
        } else {
            None
        };
        drop(compass);

        let mut last_pos = self.last_known_pos.write().await;
        let cameras: Vec<PlacedCamera> =
            self.clients.lock().await.iter().map(|c| c.camera).collect();

        let data = PositionData::new(
            pxs,
            *motion_data,
            &cameras,
            compass_value,
            last_pos.position,
            cube,
        );
        let Some(position) = Setup::calculate_position(data) else { return Ok(()); };

        let calculated_position = TimedPosition {
            position,
            start_time,
            time: Instant::now(),
            interpolated: None,
        };

        *last_pos = calculated_position;

        let mut ex = self.extrap.write().await;
        if let Some(ref mut ex) = *ex {
            ex.extrapolator.add_datapoint(calculated_position);
        };

        try_join_all(self.subscriptions.read().await.iter().filter_map(|s| {
            if let Subscriber::Position(s) = s {
                Some(s(calculated_position))
            } else {
                None
            }
        }))
        .await?;

        Ok(())
    }
}

impl<E: Send + Extrapolator, C: FnMut() -> F, F: Future<Output = Option<f64>> + Send>
    LocationServiceHandle<E, C, F>
{
    pub async fn set_motion_hint(&mut self, hint: Option<MotionHint>) {
        *self.service.motion_data.write().await = if let Some(hint) = hint {
            Some(MotionData::new(
                self.service.last_known_pos.read().await.position,
                hint,
            ))
        } else {
            None
        };
    }

    pub async fn subscribe(&mut self, action: Subscriber) {
        self.service.subscriptions.write().await.push(action);
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

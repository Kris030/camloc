use anyhow::Result;
use async_trait::async_trait;
use camloc_common::{
    hosts::{constants::MAIN_PORT, ClientData, Command, HostInfo, HostState, HostType},
    Position, TimeValidated,
};
use futures::future::try_join_all;
use std::{
    f64::NAN,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    net::UdpSocket,
    spawn,
    sync::{watch, Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    calc::{calculate_position, MotionData},
    compass::{Compass, NoCompass},
    extrapolations::{Extrapolation, LinearExtrapolation},
    MotionHint, PlacedCamera, TimedPosition,
};

struct Client {
    last_data: TimeValidated<ClientData>,
    camera: PlacedCamera,
    address: SocketAddr,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    Connect(SocketAddr, PlacedCamera),
    Disconnect(SocketAddr),
    PositionUpdate(Position),
    InfoUpdate(SocketAddr, PlacedCamera),
}

struct Inner<C, E> {
    last_known_pos: RwLock<Option<TimedPosition>>,
    motion_data: RwLock<Option<MotionData>>,
    event_tx: watch::Sender<Option<Event>>,
    cancel_token: CancellationToken,
    clients: Mutex<Vec<Client>>,
    min_camera_angle_diff: f64,
    extrapolation: RwLock<E>,
    send_events: AtomicBool,
    data_validity: Duration,
    start_time: Instant,
    compass: Mutex<C>,
}

pub struct LocationService<C, E> {
    service_task_handle: Option<JoinHandle<Result<()>>>,
    event_rx: watch::Receiver<Option<Event>>,
    service_handle: Arc<Inner<C, E>>,
    data_validity: Duration,
}

pub struct Builder<C, E> {
    last_known_pos: Option<TimedPosition>,
    motion_data: Option<MotionData>,
    cancel_token: CancellationToken,
    min_camera_angle_diff: f64,
    data_validity: Duration,
    clients: Vec<Client>,
    address: SocketAddr,
    extrapolation: E,
    compass: C,
}

impl Builder<NoCompass, LinearExtrapolation> {
    pub fn new() -> Self {
        Self {
            address: (Ipv4Addr::LOCALHOST, MAIN_PORT).into(),
            min_camera_angle_diff: 15f64.to_radians(),
            data_validity: Duration::from_millis(500),
            extrapolation: LinearExtrapolation::new(),
            cancel_token: CancellationToken::new(),
            last_known_pos: None,
            compass: NoCompass,
            motion_data: None,
            clients: vec![],
        }
    }
}
impl<C, E> Builder<C, E> {
    pub fn with_last_known_pos(mut self, v: TimedPosition) -> Self {
        self.last_known_pos = Some(v);
        self
    }
    pub fn with_motion_data(mut self, v: MotionData) -> Self {
        self.motion_data = Some(v);
        self
    }
    pub fn with_min_camera_angle_diff(mut self, v: f64) -> Self {
        self.min_camera_angle_diff = v;
        self
    }
    pub fn with_data_validity(mut self, v: Duration) -> Self {
        self.data_validity = v;
        self
    }
    pub fn with_client(
        mut self,
        last_data: TimeValidated<ClientData>,
        camera: PlacedCamera,
        address: SocketAddr,
    ) -> Self {
        self.clients.push(Client {
            last_data,
            camera,
            address,
        });
        self
    }
    pub fn with_address(mut self, v: SocketAddr) -> Self {
        self.address = v;
        self
    }
    pub fn with_cancellation_token(mut self, v: CancellationToken) -> Self {
        self.cancel_token = v;
        self
    }
    pub fn with_extrapolation<N: Extrapolation>(self, v: N) -> Builder<C, N> {
        Builder {
            extrapolation: v,
            compass: self.compass,
            address: self.address,
            clients: self.clients,
            data_validity: self.data_validity,
            min_camera_angle_diff: self.min_camera_angle_diff,
            last_known_pos: self.last_known_pos,
            motion_data: self.motion_data,
            cancel_token: self.cancel_token,
        }
    }
    pub fn with_compass<N: Compass>(self, v: N) -> Builder<N, E> {
        Builder {
            compass: v,
            address: self.address,
            clients: self.clients,
            data_validity: self.data_validity,
            min_camera_angle_diff: self.min_camera_angle_diff,
            extrapolation: self.extrapolation,
            last_known_pos: self.last_known_pos,
            motion_data: self.motion_data,
            cancel_token: self.cancel_token,
        }
    }
}

impl Default for Builder<NoCompass, LinearExtrapolation> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Compass + 'static, E: Extrapolation + 'static> Builder<C, E> {
    pub async fn start(self) -> Result<LocationService<C, E>> {
        let start_time = Instant::now();
        let udp_socket = UdpSocket::bind(self.address).await?;

        let (event_tx, event_rx) = watch::channel(None);

        let instance = Inner {
            min_camera_angle_diff: self.min_camera_angle_diff,
            last_known_pos: self.last_known_pos.into(),
            extrapolation: self.extrapolation.into(),
            motion_data: self.motion_data.into(),
            send_events: AtomicBool::new(false),
            data_validity: self.data_validity,
            cancel_token: self.cancel_token,
            clients: self.clients.into(),
            compass: self.compass.into(),
            start_time,
            event_tx,
        };

        let service_handle = Arc::new(instance);
        let service_task_handle = Some(spawn(service_handle.clone().run(udp_socket)));

        Ok(LocationService {
            data_validity: self.data_validity,
            event_rx,
            service_task_handle,
            service_handle,
        })
    }
}

impl<C: Compass, E: Extrapolation> Inner<C, E> {
    fn send_event(&self, e: Event) -> Result<()> {
        if self.send_events.load(Ordering::SeqCst) {
            self.event_tx.send(Some(e))?;
        }
        Ok(())
    }

    async fn run(self: Arc<Self>, sock: UdpSocket) -> Result<()> {
        let mut buf = [0u8; 64];

        let (cube, _organizer) = loop {
            let (len, addr) = tokio::select! {
                r = sock.recv_from(&mut buf) => r,
                _ = self.cancel_token.cancelled() => return Ok(())
            }?;

            match buf[..len].try_into() {
                Ok(Command::StartServer { cube }) => break (cube, addr),
                Ok(Command::Ping) => {
                    sock.send_to(
                        &[HostInfo {
                            host_type: HostType::Server,
                            host_state: HostState::Idle,
                        }
                        .try_into()
                        .unwrap()],
                        addr,
                    )
                    .await?;
                }
                _ => (),
            }
        };

        loop {
            let (recv_len, recv_addr) = tokio::select! {
                r = sock.recv_from(&mut buf) => r,
                _ = self.cancel_token.cancelled() => return Ok(())
            }?;

            let recv_time = Instant::now();

            match buf[..recv_len].try_into() {
                // "organizer bonk"
                Ok(Command::Ping) => {
                    sock.send_to(
                        &[HostInfo {
                            host_type: HostType::Server,
                            host_state: HostState::Running,
                        }
                        .try_into()
                        .unwrap()],
                        recv_addr,
                    )
                    .await?;
                }

                // update value
                Ok(Command::ValueUpdate(ClientData {
                    marker_id,
                    x_position,
                })) => {
                    let received_data = ClientData::new(marker_id, x_position);

                    // update client data and position if the oldest data was updated

                    let (mut oldest_data_age, mut oldest_data_index) = (self.start_time, 0);
                    let mut updated_client_index = None;

                    let mut data = vec![];

                    let mut clients = self.clients.lock().await;

                    for (i, c) in clients.iter_mut().enumerate() {
                        let data_age = c.last_data.last_changed();
                        if data_age < oldest_data_age {
                            oldest_data_age = data_age;
                            oldest_data_index = i;
                        }

                        let client_data = if c.address == recv_addr {
                            c.last_data.set_with_time(received_data, recv_time);
                            updated_client_index = Some(i);

                            Some(received_data)
                        } else {
                            c.last_data.get().copied()
                        };

                        data.push((client_data, c.camera));
                    }
                    drop(clients);

                    // if we had a legit update
                    if let Some(client_index) = updated_client_index {
                        // and it was the client that was last updated
                        if oldest_data_index == client_index {
                            self.update_position(
                                self.start_time,
                                recv_time,
                                self.min_camera_angle_diff,
                                &data[..],
                                cube,
                            )
                            .await?;
                        }
                    }
                }

                // connection request
                Ok(Command::Connect { position, fov }) => {
                    let camera = PlacedCamera::new(position, fov);
                    self.clients.lock().await.push(Client {
                        address: recv_addr,
                        camera,
                        last_data: TimeValidated::new_with_change(
                            ClientData::new(255, NAN),
                            self.data_validity,
                            recv_time - self.data_validity,
                        ),
                    });

                    self.send_event(Event::Connect(recv_addr, camera))?;
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

                            self.send_event(Event::InfoUpdate(c.address, c.camera))?;
                            break;
                        }
                    }
                }

                Ok(Command::Stop) => break,

                Ok(Command::ClientDisconnect) => {
                    let mut clients = self.clients.lock().await;
                    for i in 0..clients.len() {
                        if clients[i].address == recv_addr {
                            clients.remove(i);

                            self.send_event(Event::Disconnect(clients[i].address))?;
                            break;
                        }
                    }
                }

                _ => (),
            }
        }

        try_join_all(
            self.clients
                .lock()
                .await
                .iter()
                .map(|c| async { sock.send_to(&[Command::STOP], c.address).await }),
        )
        .await?;

        Ok(())
    }

    async fn update_position(
        self: &Arc<Self>,
        start_time: Instant,
        recv_time: Instant,
        min_camera_angle_diff: f64,
        data: &[(Option<ClientData>, PlacedCamera)],
        cube: [u8; 4],
    ) -> Result<()> {
        let mut compass = self.compass.lock().await;
        let compass_value = compass.get_value().await;
        drop(compass);

        let mut last_pos = self.last_known_pos.write().await;
        let motion_data = *self.motion_data.read().await;

        let Some(position) = calculate_position(
            min_camera_angle_diff,
            data,
            motion_data,
            compass_value,
            last_pos.map(|p| p.position),
            cube,
        ) else {
            return Ok(());
        };

        let calculated_position = TimedPosition {
            position,
            start_time,
            time: recv_time,
            extrapolated_by: None,
        };

        *last_pos = Some(calculated_position);

        let mut ex = self.extrapolation.write().await;
        ex.add_datapoint(calculated_position);

        self.send_event(Event::PositionUpdate(calculated_position.position))?;

        Ok(())
    }
}

#[async_trait]
pub trait LocationServiceTrait: Send + Sync {
    async fn set_motion_hint(&self, hint: Option<MotionHint>);
    async fn enable_events(&self);
    async fn set_events(&self, enable: bool);
    async fn disable_events(&self);
    async fn get_event(&mut self) -> Result<Event>;
    async fn get_position(&self) -> Option<Position>;
    async fn stop(self) -> Result<()>;
}

#[async_trait]
impl<C: Compass, E: Extrapolation> LocationServiceTrait for LocationService<C, E> {
    async fn set_motion_hint(&self, hint: Option<MotionHint>) {
        let pos_handle = self.service_handle.last_known_pos.read().await;
        let Some(pos) = *pos_handle else {
            return;
        };
        drop(pos_handle);

        let new_hint = hint.map(|hint| MotionData::new(pos.position, hint));
        *self.service_handle.motion_data.write().await = new_hint;
    }

    async fn enable_events(&self) {
        self.set_events(true).await
    }
    async fn set_events(&self, enable: bool) {
        self.service_handle
            .send_events
            .store(enable, Ordering::SeqCst);
    }
    async fn disable_events(&self) {
        self.set_events(false).await
    }

    async fn get_event(&mut self) -> Result<Event> {
        Ok(self.event_rx.wait_for(|e| e.is_some()).await?.unwrap())
    }

    async fn get_position(&self) -> Option<Position> {
        let Some(pos) = *self.service_handle.last_known_pos.read().await else {
            return None;
        };

        if pos.position.x.is_nan() || pos.position.y.is_nan() {
            return None;
        }

        let now = Instant::now();

        let ex = self.service_handle.extrapolation.read().await;
        if now > pos.time + self.data_validity {
            return None;
        }

        ex.extrapolate(now)
    }

    async fn stop(mut self) -> Result<()> {
        let Some(h) = self.service_task_handle.take() else {
            return Err(anyhow::Error::msg("Service background task already joined"));
        };
        drop(self);
        h.await?
    }
}

impl<C, E> Drop for LocationService<C, E> {
    fn drop(&mut self) {
        self.service_handle.cancel_token.cancel();
    }
}

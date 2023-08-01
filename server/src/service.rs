use anyhow::Result;
use camloc_common::{
    hosts::{constants::MAIN_PORT, ClientData, Command, HostInfo, HostState, HostType},
    Position, TimeValidated,
};
use futures::future::try_join_all;
use std::{
    f64::NAN,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    net::{ToSocketAddrs, UdpSocket},
    spawn,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{
    calc::{calculate_position, MotionData},
    compass::{Compass, NoCompass},
    extrapolations::{Extrapolation, NoExtrapolation},
    MotionHint, PlacedCamera, TimedPosition,
};

struct Client {
    last_data: TimeValidated<ClientData>,
    camera: PlacedCamera,
    address: SocketAddr,
}

pub trait Subscriber: Send + Sync {
    fn handle_event(&mut self, event: Event);
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Event {
    Connect(SocketAddr, PlacedCamera),
    Disconnect(SocketAddr),
    PositionUpdate(Position),
    InfoUpdate(SocketAddr, PlacedCamera),
}

struct LocationServiceInner<C, E> {
    last_known_pos: RwLock<Option<TimedPosition>>,
    motion_data: RwLock<Option<MotionData>>,
    cancel_token: CancellationToken,
    extrapolation: RwLock<E>,
    compasses: Mutex<Vec<C>>,
    clients: Mutex<Vec<Client>>,
    min_camera_angle_diff: f64,
    data_validity: Duration,
    start_time: Instant,
}

pub struct LocationService<C, E> {
    service_task_handle: Option<JoinHandle<Result<()>>>,
    service_handle: Arc<LocationServiceInner<C, E>>,
    data_validity: Duration,
}

pub struct Builder<A, C, E> {
    last_known_pos: Option<TimedPosition>,
    motion_data: Option<MotionData>,
    min_camera_angle_diff: f64,
    data_validity: Duration,
    clients: Vec<Client>,
    compasses: Vec<C>,
    extrapolation: E,
    address: A,
}

impl<A: ToSocketAddrs> Builder<A, NoCompass, NoExtrapolation> {
    pub fn new(address: A) -> Self {
        Self {
            min_camera_angle_diff: 15f64.to_radians(),
            data_validity: Duration::from_millis(500),
            extrapolation: NoExtrapolation,
            last_known_pos: None,
            motion_data: None,
            compasses: vec![],
            clients: vec![],
            address,
        }
    }
}

impl<A: ToSocketAddrs, C: Compass, E: Extrapolation> Builder<A, C, E> {
    pub async fn start(self) -> Result<LocationService<C, E>> {
        let start_time = Instant::now();
        let udp_socket = UdpSocket::bind(self.address).await?;

        let instance = LocationServiceInner {
            min_camera_angle_diff: self.min_camera_angle_diff,
            last_known_pos: self.last_known_pos.into(),
            extrapolation: self.extrapolation.into(),
            cancel_token: CancellationToken::new(),
            motion_data: self.motion_data.into(),
            data_validity: self.data_validity,
            compasses: self.compasses.into(),
            clients: self.clients.into(),
            start_time,
        };

        let service_handle = Arc::new(instance);
        let service_task_handle = Some(spawn(service_handle.clone().run(udp_socket)));

        Ok(LocationService {
            service_task_handle,
            service_handle,
            data_validity: self.data_validity,
        })
    }
}

impl<C: Compass, E: Extrapolation> LocationServiceInner<C, E> {
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

                    for s in self.subscriptions.write().await.iter_mut() {
                        s.handle_event(Event::Connect(recv_addr, camera));
                    }
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

                            for s in self.subscriptions.write().await.iter_mut() {
                                s.handle_event(Event::InfoUpdate(c.address, c.camera));
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

                            for s in self.subscriptions.write().await.iter_mut() {
                                s.handle_event(Event::Disconnect(clients[i].address));
                            }
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
        let mut compasses = self.compasses.lock().await;
        let compass_count = compasses.len();

        let compass_value: Option<f64> = 'avg: {
            if compass_count == 0 {
                break 'avg None;
            }

            let mut compass_values = vec![0.; compasses.len()];
            for compass in compasses.iter_mut() {
                if let Some(v) = compass.get_value().await {
                    compass_values.push(v);
                }
            }

            Some(compass_values.iter().copied().sum::<f64>() / compass_count as f64)
        };
        drop(compasses);

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

        let mut ex = self.extrap.write().await;
        if let Some(ref mut ex) = *ex {
            ex.add_datapoint(calculated_position);
        };

        for s in self.subscriptions.write().await.iter_mut() {
            s.handle_event(Event::PositionUpdate(calculated_position.position));
        }

        Ok(())
    }
}

impl<C: Compass, E: Extrapolation> LocationService<C, E> {
    pub async fn set_motion_hint(&self, hint: Option<MotionHint>) {
        let pos_handle = self.service_handle.last_known_pos.read().await;
        let Some(pos) = *pos_handle else {
            return;
        };
        drop(pos_handle);

        let new_hint = hint.map(|hint| MotionData::new(pos.position, hint));
        *self.service_handle.motion_data.write().await = new_hint;
    }

    pub async fn subscribe(&self) {
        self.service_handle
            .subscriptions
            .write()
            .await
            .push(Box::new(action));
    }

    pub async fn get_position(&self) -> Option<Position> {
        let Some(pos) = *self.service_handle.last_known_pos.read().await else {
            return None;
        };

        if pos.position.x.is_nan() || pos.position.y.is_nan() {
            return None;
        }

        let now = Instant::now();

        let ex = self.service_handle.extrapolation.read().await;
        if let Some(x) = &*ex {
            if now > pos.time + self.data_validity {
                return None;
            }

            x.extrapolate(now)
        } else {
            Some(pos.position)
        }
    }

    pub async fn stop(mut self) -> Result<()> {
        let Some(h) = self.service_task_handle.take() else {
            return Err(anyhow::Error::msg("Service background task already joined"));
        };
        drop(self);
        h.await?
    }

    pub async fn set_extrapolation(&self, extrapolation: Option<impl Extrapolation + 'static>) {
        *self.service_handle.extrap.write().await =
            extrapolation.map(|e| Box::new(e) as Box<dyn Extrapolation>);
    }

    pub async fn add_compass(&self, compass: impl Compass + Send + 'static) {
        self.service_handle
            .compasses
            .lock()
            .await
            .push(Box::new(compass));
    }

    pub async fn modify_compasses(&self, action: impl FnOnce(&mut Vec<Box<dyn Compass>>)) {
        action(&mut *self.service_handle.compasses.lock().await);
    }
}

impl Drop for LocationService {
    fn drop(&mut self) {
        self.service_handle.cancel_token.cancel();
    }
}

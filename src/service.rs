use anyhow::Result;
use camloc_common::{
    hosts::{ClientData, Command, HostInfo, HostState, HostType},
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
    net::UdpSocket,
    spawn,
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

use crate::{
    calc::{calculate_position, MotionData, PositionData},
    compass::Compass,
    extrapolations::Extrapolation,
    MotionHint, PlacedCamera, TimedPosition,
};

struct Client {
    last_data: TimeValidated<ClientData>,
    camera: PlacedCamera,
    address: SocketAddr,
}
impl Client {
    fn new(
        address: SocketAddr,
        camera: PlacedCamera,
        last_data: TimeValidated<ClientData>,
    ) -> Self {
        Self {
            last_data,
            address,
            camera,
        }
    }
}

pub trait Subscriber: Send + Sync {
    fn handle_event(&mut self, event: Event);
}

#[derive(Clone, Copy)]
pub enum Event {
    Connect(SocketAddr, PlacedCamera),
    Disconnect(SocketAddr),
    PositionUpdate(TimedPosition),
    InfoUpdate(SocketAddr, PlacedCamera),
}

struct LocationService {
    extrap: RwLock<Option<Box<dyn Extrapolation + Send>>>,
    motion_data: RwLock<Option<MotionData>>,
    subscriptions: RwLock<Vec<Box<dyn Subscriber + Send>>>,
    last_known_pos: RwLock<TimedPosition>,
    clients: Mutex<Vec<Client>>,
    start_time: RwLock<Instant>,
    compasses: Mutex<Vec<Box<dyn Compass + Send + 'static>>>,
    running: RwLock<bool>,
}

pub struct LocationServiceHandle {
    service_task_handle: Option<JoinHandle<Result<()>>>,
    service_handle: Arc<LocationService>,
    data_validity: Duration,
}

impl Drop for LocationServiceHandle {
    fn drop(&mut self) {
        let handle = self
            .service_task_handle
            .take()
            .expect("Service task handle should always be Some");

        let running = self.service_handle.running.write();
        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut running = running.await;
                *running = false;
                drop(running);

                handle.await.is_ok()
            })
        });

        if !res {
            panic!("Should always be able join the task");
        }
    }
}

pub async fn start(
    extrapolation: Option<impl Extrapolation + Send + 'static>,
    port: u16,
    compasses: impl IntoIterator<Item = Box<dyn Compass + Send + 'static>>,
    data_validity: Duration,
) -> Result<LocationServiceHandle> {
    let start_time = Instant::now();
    let udp_socket = UdpSocket::bind(("0.0.0.0", port)).await?;

    let instance = LocationService {
        last_known_pos: TimedPosition {
            position: Position::new(NAN, NAN, NAN),
            extrapolated_by: None,
            time: start_time,
            start_time,
        }
        .into(),
        start_time: start_time.into(),
        subscriptions: vec![].into(),
        extrap: RwLock::new(extrapolation.map(|e| Box::new(e) as Box<dyn Extrapolation + Send>)),
        motion_data: None.into(),
        compasses: Mutex::new(compasses.into_iter().collect()),
        clients: vec![].into(),
        running: true.into(),
    };

    let service_handle = Arc::new(instance);
    let service_task_handle = Some(spawn(service_handle.clone().run(
        udp_socket,
        start_time,
        data_validity,
    )));

    Ok(LocationServiceHandle {
        service_task_handle,
        service_handle,
        data_validity,
    })
}

impl LocationService {
    async fn run(
        self: Arc<Self>,
        udp_socket: UdpSocket,
        start_time: Instant,
        data_validity: Duration,
    ) -> Result<()> {
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
            let (len, addr) = recv_result?;

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
                        .await?;
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
            let (recv_len, recv_addr) = recv_result?;

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
                        .await?;
                }

                // update value
                Ok(Command::ValueUpdate(ClientData {
                    marker_id,
                    x_position,
                })) => {
                    let received_data = ClientData::new(marker_id, x_position);

                    // update client data and position if the oldest data was updated

                    let (mut oldest_data_age, mut oldest_data_index) = (start_time, 0);
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
                            self.update_position(start_time, recv_time, &data[..], cube)
                                .await?;
                        }
                    }
                }

                // connection request
                Ok(Command::Connect { position, fov }) => {
                    let camera = PlacedCamera::new(position, fov);
                    self.clients.lock().await.push(Client::new(
                        recv_addr,
                        camera,
                        TimeValidated::new_with_change(
                            ClientData::new(255, NAN),
                            data_validity,
                            recv_time - data_validity,
                        ),
                    ));

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
                .map(|c| async { udp_socket.send_to(&[Command::STOP], c.address).await }),
        )
        .await?;

        Ok(())
    }

    async fn update_position(
        self: &Arc<Self>,
        start_time: Instant,
        recv_time: Instant,
        data: &[(Option<ClientData>, PlacedCamera)],
        cube: [u8; 4],
    ) -> Result<()> {
        let mut compasses = self.compasses.lock().await;
        let compass_count = compasses.len();

        let compass_value: Option<f64> = 'avg: {
            if compass_count == 0 {
                break 'avg None;
            }

            let mut compass_values = Vec::with_capacity(compasses.len());
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

        let data = PositionData::new(data, motion_data, compass_value, last_pos.position, cube);
        let Some(position) = calculate_position(&data) else { return Ok(()); };

        let calculated_position = TimedPosition {
            position,
            start_time,
            time: recv_time,
            extrapolated_by: None,
        };

        *last_pos = calculated_position;

        let mut ex = self.extrap.write().await;
        if let Some(ref mut ex) = *ex {
            ex.add_datapoint(calculated_position);
        };

        for s in self.subscriptions.write().await.iter_mut() {
            s.handle_event(Event::PositionUpdate(calculated_position));
        }

        Ok(())
    }
}

impl LocationServiceHandle {
    pub async fn set_motion_hint(&self, hint: Option<MotionHint>) {
        *self.service_handle.motion_data.write().await = if let Some(hint) = hint {
            Some(MotionData::new(
                self.service_handle.last_known_pos.read().await.position,
                hint,
            ))
        } else {
            None
        };
    }

    pub async fn subscribe(&self, action: impl Subscriber + Send + 'static) {
        self.service_handle
            .subscriptions
            .write()
            .await
            .push(Box::new(action));
    }

    pub async fn modify_subscriptions(
        &self,
        action: impl FnOnce(&mut Vec<Box<dyn Subscriber + Send>>),
    ) {
        action(&mut *self.service_handle.subscriptions.write().await);
    }

    pub async fn get_position(&self) -> Option<TimedPosition> {
        if !*(self.service_handle.running.read().await) {
            return None;
        }

        let pos = *self.service_handle.last_known_pos.read().await;
        if pos.position.x.is_nan() || pos.position.y.is_nan() {
            return None;
        }

        let start_time = *self.service_handle.start_time.read().await;
        let now = Instant::now();

        let ex = self.service_handle.extrap.read().await;
        if let Some(x) = &*ex {
            if now > pos.time + self.data_validity {
                return None;
            }

            x.extrapolate(now).map(|extrapolated| TimedPosition {
                position: extrapolated,
                start_time,
                time: now,
                extrapolated_by: x.get_last_datapoint().map(|p| now - p.time),
            })
        } else {
            Some(pos)
        }
    }

    pub fn stop(self) {
        drop(self)
    }

    pub async fn is_running(&self) -> bool {
        *self.service_handle.running.read().await
    }

    pub async fn set_extrapolation(&self, extrapolation: Option<impl Extrapolation + 'static>) {
        *self.service_handle.extrap.write().await =
            extrapolation.map(|e| Box::new(e) as Box<dyn Extrapolation + Send>);
    }

    pub async fn add_compass(&self, compass: impl Compass + Send + 'static) {
        self.service_handle
            .compasses
            .lock()
            .await
            .push(Box::new(compass));
    }

    pub async fn modify_compasses(&self, action: impl FnOnce(&mut Vec<Box<dyn Compass + Send>>)) {
        action(&mut *self.service_handle.compasses.lock().await);
    }
}

use camloc_common::{
    cv::{self, FoundBoard},
    hosts::constants::{MAIN_PORT, ORGANIZER_STARTER_PORT},
    hosts::{Command, HostInfo, HostState, HostType},
    Position,
};
use opencv::{self, imgcodecs, objdetect::CharucoBoard, prelude::*};
use std::{
    io::{Read, Write},
    mem::size_of,
    net::{IpAddr, Ipv4Addr, TcpListener, UdpSocket},
    time::{Duration, Instant},
};
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum GetServerError {
    #[error("No running server")]
    NoServer,

    #[error("Multiple running servers ({0})")]
    Multiple(usize),
}

#[derive(ThisError, Debug)]
pub enum StopError {
    #[error("host not running")]
    NotRunning(Host),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    GetServer(#[from] GetServerError),
}

#[derive(ThisError, Debug)]
pub enum InfoUpdateError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    GetServer(#[from] GetServerError),
}
#[derive(ThisError, Debug)]
pub enum StartServerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    GetServer(#[from] GetServerError),
}

#[derive(ThisError, Debug)]
pub enum StartError<I> {
    #[error(transparent)]
    StartServer(#[from] StartServerError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("opencv error: {0}")]
    OpenCV(#[from] opencv::Error),

    #[error(transparent)]
    GetServer(#[from] GetServerError),

    #[error(transparent)]
    GetImage(#[from] GetImageError),

    #[error("interface error: {0}")]
    Interface(I),
}

#[derive(ThisError, Debug)]
pub enum GetImageError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("opencv error: {0}")]
    OpenCV(#[from] opencv::Error),
}

#[derive(ThisError, Debug)]
pub enum ScanError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Host {
    info: HostInfo,
    ip: Ipv4Addr,
}
impl std::fmt::Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ip = &self.ip;
        match &self.info.host_type {
            HostType::Client { calibrated } => {
                write!(f, "CLIENT {ip}")?;
                if *calibrated {
                    write!(f, " CALIBRATED")?;
                }
            }
            HostType::ConfiglessClient => write!(f, "PHONE {ip}")?,
            HostType::Server => write!(f, "SERVER {ip}")?,
        }
        write!(f, " {:?}", self.info.host_state)?;
        Ok(())
    }
}

impl Host {
    pub fn info(&self) -> HostInfo {
        self.info
    }

    pub fn ip(&self) -> Ipv4Addr {
        self.ip
    }
}

pub struct Organizer<'a, const BUFFER_SIZE: usize> {
    buffer: &'a mut [u8; BUFFER_SIZE],
    server_sock: TcpListener,
    hosts: Vec<Host>,
    sock: UdpSocket,
    cube: [u8; 4],
}

pub trait CalibrationInterface {
    type Parent: OrganizerInterface;

    fn get_board_size(&self) -> Result<(u8, u8), <Self::Parent as OrganizerInterface>::Error>;

    fn keep_image(
        &self,
        img: &Mat,
        board: &FoundBoard,
    ) -> Result<bool, <Self::Parent as OrganizerInterface>::Error>;

    fn board_not_found(&self, img: &Mat)
        -> Result<(), <Self::Parent as OrganizerInterface>::Error>;

    fn more(&self) -> Result<bool, <Self::Parent as OrganizerInterface>::Error>;

    fn select_camera_position(
        self,
        fov: f64,
    ) -> Result<Position, <Self::Parent as OrganizerInterface>::Error>;
}

pub trait ImageStreamInterface {
    type Parent: OrganizerInterface;

    fn show(&self, img: &Mat) -> Result<(), <Self::Parent as OrganizerInterface>::Error>;

    fn more(&self) -> Result<bool, <Self::Parent as OrganizerInterface>::Error>;

    fn select_camera_position(
        self,
        fov: f64,
    ) -> Result<Position, <Self::Parent as OrganizerInterface>::Error>;
}

pub trait OrganizerInterface {
    type CalibrationInterface: CalibrationInterface<Parent = Self>;
    type ImageStreamInterface: ImageStreamInterface<Parent = Self>;
    type Error;

    fn start_image_stream(self) -> Result<Self::ImageStreamInterface, Self::Error>;
    fn start_calibration(self) -> Result<Self::CalibrationInterface, Self::Error>;
}

enum ImgLoopState<C, S> {
    Calibrating {
        board: CharucoBoard,
        images: Vec<Mat>,
        interface: C,
    },
    Showing {
        interface: S,
    },
}

impl<'o, const BUFFER_SIZE: usize> Organizer<'o, BUFFER_SIZE> {
    pub fn start(buffer: &'o mut [u8; BUFFER_SIZE], cube: [u8; 4]) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(("0.0.0.0", 0))?;
        sock.set_broadcast(true)?;
        sock.set_read_timeout(Some(TIMEOUT_DURATION))?;

        Ok(Self {
            server_sock: TcpListener::bind(("0.0.0.0", ORGANIZER_STARTER_PORT))?,
            sock,
            hosts: vec![],
            buffer,
            cube,
        })
    }

    pub fn update_info(
        &mut self,
        host: Host,
        position: camloc_common::Position,
        fov: Option<f64>,
    ) -> Result<(), InfoUpdateError> {
        self.sock.send_to(
            &Into::<Vec<u8>>::into(Command::InfoUpdate {
                client_ip: &host.ip.to_string(),
                position,
                fov,
            }),
            (self.get_server()?.ip, MAIN_PORT),
        )?;
        Ok(())
    }

    pub fn hosts(&self) -> &[Host] {
        &self.hosts
    }

    pub fn get_server(&self) -> Result<&Host, GetServerError> {
        let mut si = Err(GetServerError::NoServer);

        for h in &self.hosts {
            if matches!(
                h.info,
                HostInfo {
                    host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                    host_state: HostState::Idle
                },
            ) {
                si = match si {
                    Ok(_) => Err(GetServerError::Multiple(1)),
                    Err(GetServerError::NoServer) => Ok(h),
                    Err(GetServerError::Multiple(n)) => Err(GetServerError::Multiple(n + 1)),
                };
            }
        }

        si
    }

    pub fn start_server(&mut self) -> Result<(), StartServerError> {
        self.sock.send_to(
            &Into::<Vec<u8>>::into(Command::StartServer { cube: self.cube }),
            (self.get_server()?.ip, MAIN_PORT),
        )?;
        Ok(())
    }

    pub fn start_host<I: OrganizerInterface>(
        &mut self,
        host: Host,
        interface: I,
    ) -> Result<(), StartError<I::Error>> {
        if host.info.host_type == HostType::Server {
            self.start_server()?;
            return Ok(());
        }

        let addr = (host.ip, MAIN_PORT);

        self.sock.send_to(&[Command::START], addr)?;

        // wait for connection on the serversocket
        let mut s = loop {
            let (s, a) = self.server_sock.accept()?;
            if addr.0 == a.ip() {
                break s;
            }
        };

        let uncalibrated = match host.info {
            HostInfo {
                host_type: HostType::Client { calibrated },
                ..
            } => !calibrated,
            HostInfo {
                host_type: HostType::ConfiglessClient,
                ..
            } => false,
            HostInfo {
                host_type: HostType::Server,
                ..
            } => unreachable!(),
        };

        let mut uncalibrated: ImgLoopState<
            <I as OrganizerInterface>::CalibrationInterface,
            <I as OrganizerInterface>::ImageStreamInterface,
        > = if uncalibrated {
            let interface = interface
                .start_calibration()
                .map_err(StartError::Interface)?;
            let (width, height) = interface.get_board_size().map_err(StartError::Interface)?;
            ImgLoopState::Calibrating {
                board: cv::generate_board(width, height)?,
                images: vec![],
                interface,
            }
        } else {
            ImgLoopState::Showing {
                interface: interface
                    .start_image_stream()
                    .map_err(StartError::Interface)?,
            }
        };

        loop {
            s.write_all(&[Command::REQUEST_IMAGE])?;

            let img = self.get_image(&mut s)?;

            match &mut uncalibrated {
                ImgLoopState::Calibrating {
                    board,
                    images,
                    interface,
                } => {
                    let detection = cv::find_board(&img, board, false)?;

                    if let Some(fb) = detection {
                        if interface
                            .keep_image(&img, &fb)
                            .map_err(StartError::Interface)?
                        {
                            images.push(img);
                        }
                    } else {
                        interface
                            .board_not_found(&img)
                            .map_err(StartError::Interface)?;
                    }
                    if !images.is_empty() && !interface.more().map_err(StartError::Interface)? {
                        break;
                    }
                }

                ImgLoopState::Showing { interface } => {
                    interface.show(&img).map_err(StartError::Interface)?;
                    if !interface.more().map_err(StartError::Interface)? {
                        break;
                    }
                }
            }
        }
        s.write_all(&[Command::IMAGES_DONE])?;

        let server_ip = self.get_server()?.ip.to_string();
        let ip_bytes = server_ip.as_bytes();
        let ip_len = ip_bytes.len() as u16;

        let (pos, calib) = match uncalibrated {
            ImgLoopState::Calibrating {
                interface,
                board,
                images,
            } => {
                let calib = cv::calibrate(&board, &images, images[0].size()?)?;

                let pos = interface
                    .select_camera_position(calib.horizontal_fov)
                    .map_err(StartError::Interface)?;

                (pos, Some(calib))
            }

            ImgLoopState::Showing { interface } => {
                let fov = &mut self.buffer[..size_of::<f64>()];
                s.read_exact(fov)?;
                let fov = f64::from_be_bytes(fov.try_into().unwrap());

                let pos = interface
                    .select_camera_position(fov)
                    .map_err(StartError::Interface)?;

                (pos, None)
            }
        };

        s.write_all(pos.x.to_be_bytes().as_slice())?;
        s.write_all(pos.y.to_be_bytes().as_slice())?;
        s.write_all(pos.rotation.to_be_bytes().as_slice())?;
        s.write_all(ip_len.to_be_bytes().as_slice())?;
        s.write_all(ip_bytes)?;

        if let Some(calib) = calib {
            s.write_all(calib.to_be_bytes().as_slice())?;
        }

        s.write_all(&self.cube.map(u8::to_be))?;

        Ok(())
    }

    pub fn stop_host(&mut self, host: Host) -> Result<(), StopError> {
        if !matches!(
            host.info,
            HostInfo {
                host_state: HostState::Running,
                ..
            }
        ) {
            return Err(StopError::NotRunning(host));
        }

        self.sock.send_to(&[Command::STOP], (host.ip, MAIN_PORT))?;
        self.hosts
            .remove(self.hosts.iter().position(|h| *h == host).unwrap());

        Ok(())
    }

    pub fn scan(&mut self) -> Result<(), ScanError> {
        let till = Instant::now() + WAIT_DURATION;
        self.sock.send_to(
            &[Command::PING],
            (IpAddr::V4(Ipv4Addr::BROADCAST), MAIN_PORT),
        )?;

        let mut hit_hosts = vec![false; self.hosts.len()];

        'loopy: while Instant::now() < till {
            let addr = match self.sock.recv_from(self.buffer) {
                Ok((1, addr)) => addr,
                Ok(_) => continue,

                Err(e) => Err(e)?,
            };

            let IpAddr::V4(ip) = addr.ip() else {
                continue;
            };
            let Ok(info) = self.buffer[0].try_into() else {
                continue 'loopy;
            };

            let h = self
                .hosts
                .iter_mut()
                .zip(hit_hosts.iter_mut())
                .find(|(h, _)| h.ip == ip);

            if let Some((h, hit)) = h {
                *hit = true;
                h.info = info;
            } else {
                self.hosts.push(Host { info, ip });
            }
        }

        for (h, hit) in self.hosts.iter_mut().zip(hit_hosts.iter()) {
            if *hit {
                continue;
            }
            h.info = match h.info {
                HostInfo {
                    host_type: HostType::ConfiglessClient,
                    ..
                } => HostInfo {
                    host_type: HostType::ConfiglessClient,
                    host_state: HostState::Unreachable,
                },
                HostInfo {
                    host_type: HostType::Client { calibrated },
                    ..
                } => HostInfo {
                    host_type: HostType::Client { calibrated },
                    host_state: HostState::Unreachable,
                },

                HostInfo {
                    host_type: HostType::Server,
                    ..
                } => HostInfo {
                    host_type: HostType::Server,
                    host_state: HostState::Unreachable,
                },
            };
        }

        Ok(())
    }

    fn get_image(&mut self, r: &mut impl Read) -> Result<Mat, GetImageError> {
        r.read_exact(&mut self.buffer[..size_of::<u64>()])?;
        let len = u64::from_be_bytes(self.buffer[..size_of::<u64>()].try_into().unwrap()) as usize;

        let mut buffer = opencv::core::Vector::from_elem(0, len);

        r.read_exact(&mut buffer.as_mut_slice()[..len])?;

        Ok(imgcodecs::imdecode(&buffer, imgcodecs::IMREAD_COLOR)?)
    }
}

const TIMEOUT_DURATION: Duration = Duration::from_millis(500);
const WAIT_DURATION: Duration = Duration::from_millis(TIMEOUT_DURATION.as_millis() as u64 * 4);

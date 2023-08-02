mod aruco;
mod util;

use crate::aruco::Aruco;
use anyhow::{anyhow, Result};
use camloc_common::{
    cv::FullCameraInfo,
    hosts::{
        constants::{MAIN_PORT, ORGANIZER_STARTER_PORT},
        Command, HostInfo, HostState, HostType,
    },
    Position,
};
use opencv::{
    self, core, highgui,
    prelude::*,
    videoio::{self, VideoCapture},
};
use std::{
    fs::File,
    io::{ErrorKind, Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    time::Duration,
};

const BUF_SIZE: usize = 2048;

struct Config {
    calibration: FullCameraInfo,
    server: SocketAddr,
    cube: [u8; 4],
}

impl Config {
    fn to_connection_request(&self, position: Position) -> Option<[u8; 33]> {
        Into::<Vec<u8>>::into(Command::Connect {
            position,
            fov: self.calibration.horizontal_fov,
        })
        .try_into()
        .ok()
    }

    fn from_organizer(
        r: &mut impl Read,
        cached_calibration: &Option<FullCameraInfo>,
    ) -> Result<(Self, Position)> {
        let mut buf = vec![0; 26];
        r.read_exact(&mut buf)?;

        let x = f64::from_be_bytes(buf[0..8].try_into()?);
        let y = f64::from_be_bytes(buf[8..16].try_into()?);
        let rotation = f64::from_be_bytes(buf[16..24].try_into()?);

        let ip_len = u16::from_be_bytes(buf[24..26].try_into()?) as usize;

        buf.resize(ip_len, 0);
        r.read_exact(&mut buf)?;

        let ip = String::from_utf8(buf)?;

        let server = SocketAddr::new(ip.parse()?, MAIN_PORT);

        let calibration = if let Some(c) = cached_calibration {
            c.clone()
        } else {
            FullCameraInfo::from_be_bytes(r)?
        };

        let mut cube = [0; 4];
        r.read_exact(&mut cube)?;

        Ok((
            Self {
                calibration,
                server,
                cube: cube.map(u8::from_be),
            },
            Position::new(x, y, rotation),
        ))
    }
}

fn main() -> Result<()> {
    let args = {
        use clap::Parser;

        /// The camloc client
        #[derive(Parser)]
        struct Args {
            /// The camera index to use
            #[arg(long, default_value_t = 0u16)]
            camera_index: u16,

            /// Calibration cache file
            #[arg(long, default_value = ".calib")]
            calibration_cache: String,

            /// Show what's happening
            #[arg(short, long, default_value_t = false)]
            gui: bool,
        }

        Args::parse()
    };

    // TODO: ctrl+c handling
    // let (tx, rx) = std::sync::mpsc::channel::<()>();
    // ctrlc::set_handler(move || {
    //     let _ = tx.send(());
    // })
    // .map_err(|_| "Couldn't set ctr+c handler")?;
    // if rx.recv_timeout(Duration::from_millis(1)).is_ok() {

    // }

    let cached_calibration = if let Ok(mut f) = File::open(&args.calibration_cache) {
        println!("Found calibration file");
        FullCameraInfo::from_be_bytes(&mut f).ok()
    } else {
        None
    };

    let mut frame = Mat::default();

    let socket = UdpSocket::bind(("0.0.0.0", MAIN_PORT))?;
    let mut buf = [0; BUF_SIZE];

    'outer_loop: loop {
        println!("Waiting for organizer...");

        // wait for organizer ping / start
        let organizer = loop {
            let (len, addr) = socket.recv_from(&mut buf)?;

            match buf[..len].try_into() {
                Ok(Command::Start) => break addr,
                Ok(Command::Ping) => {
                    socket.send_to(
                        &[TryInto::<u8>::try_into(HostInfo {
                            host_type: HostType::Client {
                                calibrated: cached_calibration.is_some(),
                            },
                            host_state: HostState::Idle,
                        })
                        .unwrap()],
                        addr,
                    )?;
                }
                _ => continue,
            }
        };

        let mut cam = VideoCapture::new(args.camera_index as i32, videoio::CAP_ANY)?;

        // recieve camera info and server ip
        let (config, pos) = match get_config(
            &mut buf,
            &organizer.ip(),
            &mut cam,
            &mut frame,
            &cached_calibration,
        ) {
            Ok(c) => c,
            Err(e) => {
                println!("Couldn't get config from organizer because: {e}");
                continue 'outer_loop;
            }
        };

        // connect to server
        socket.send_to(&config.to_connection_request(pos).unwrap(), config.server)?;

        inner_loop(&socket, &mut cam, config, &mut buf, &mut frame, args.gui)?;
    }
}

fn inner_loop(
    socket: &UdpSocket,
    cam: &mut VideoCapture,
    config: Config,
    buf: &mut [u8],
    mut frame: &mut Mat,
    gui: bool,
) -> Result<()> {
    let mut draw = Mat::default();
    let mut aruco = Aruco::new(config.cube)?;

    if gui {
        highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    }

    let stopped_by_server = loop {
        let read_timeout = socket.read_timeout()?;

        socket.set_read_timeout(Some(Duration::from_millis(1)))?;

        match socket.recv_from(buf) {
            Ok((len, addr)) => match buf[..len].try_into() {
                Ok(Command::Stop) => break true,

                Ok(Command::Ping) => {
                    socket.send_to(
                        &[HostInfo {
                            host_type: HostType::Client { calibrated: true },
                            host_state: HostState::Running,
                        }
                        .try_into()
                        .unwrap()],
                        addr,
                    )?;
                }

                _ => (),
            },
            Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => (),
            Err(_) => Err(anyhow!("Error while receiving command"))?,
        }

        socket.set_read_timeout(read_timeout)?;

        if gui && highgui::wait_key(10)? == 113 {
            break false;
        }

        // find & send x value
        cam.read(&mut frame)?;

        if gui {
            frame.copy_to(&mut draw)?;
        }

        if let Some(data) = aruco.detect(frame, if gui { Some(&mut draw) } else { None })? {
            socket.send_to(
                &Into::<Vec<u8>>::into(Command::ValueUpdate(data)),
                config.server,
            )?;
        }

        if gui {
            highgui::imshow("videocap", &draw)?;
        }
    };

    if gui {
        highgui::destroy_all_windows()?;
    }

    if !stopped_by_server {
        socket.send_to(&[Command::CLIENT_DISCONNECT], config.server)?;
    }

    Ok(())
}

fn get_config(
    buf: &mut [u8],
    organizer: &IpAddr,
    cam: &mut VideoCapture,
    mut frame: &mut Mat,
    cached_calibration: &Option<FullCameraInfo>,
) -> Result<(Config, Position)> {
    let mut s = TcpStream::connect((*organizer, ORGANIZER_STARTER_PORT))?;

    'image_loop: loop {
        'request_wait_loop: loop {
            s.read_exact(&mut buf[..1])?;

            match buf[..1].try_into() {
                Ok(Command::RequestImage) => break 'request_wait_loop,
                Ok(Command::ImagesDone) => break 'image_loop,
                _ => (),
            }
        }

        cam.read(&mut frame)?;

        let mut image_buffer = core::Vector::new();
        opencv::imgcodecs::imencode(".jpg", frame, &mut image_buffer, &core::Vector::new())?;

        let total = image_buffer.len() as u64;
        s.write_all(&total.to_be_bytes())?;
        s.write_all(image_buffer.as_slice())?;
    }

    if let Some(c) = cached_calibration {
        s.write_all(c.horizontal_fov.to_be_bytes().as_slice())?;
    }

    // recieve camera info and server ip
    Config::from_organizer(&mut s, cached_calibration)
}

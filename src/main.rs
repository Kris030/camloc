mod aruco;
mod util;

use crate::aruco::Aruco;
use camloc_common::opencv::{
    self, core, highgui,
    prelude::*,
    videoio::{self, VideoCapture},
};
use camloc_common::{
    cv::FullCameraInfo,
    hosts::{
        constants::{MAIN_PORT, ORGANIZER_STARTER_PORT},
        Command, HostInfo, HostState, HostType,
    },
    Position,
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
    ) -> Result<(Self, Position), &'static str> {
        let mut buf = vec![0; 26];
        r.read_exact(&mut buf)
            .map_err(|_| "Couldn't read config x, y, rotation, ip_len")?;

        let x = f64::from_be_bytes(buf[0..8].try_into().unwrap());
        let y = f64::from_be_bytes(buf[8..16].try_into().unwrap());
        let rotation = f64::from_be_bytes(buf[16..24].try_into().unwrap());

        let ip_len = u16::from_be_bytes(buf[24..26].try_into().unwrap()) as usize;

        buf.resize(ip_len, 0);
        r.read_exact(&mut buf).map_err(|_| "Couldn't read ip")?;

        let ip = String::from_utf8(buf).map_err(|_| "Ip isn't valid utf-8")?;

        let server = SocketAddr::new(ip.parse().map_err(|_| "Couldn't parse ip")?, MAIN_PORT);

        let calibration = if let Some(c) = cached_calibration {
            c.clone()
        } else {
            FullCameraInfo::from_be_bytes(r).map_err(|_| "Couldn't get camera info")?
        };

        let mut cube = [0; 4];
        r.read_exact(&mut cube).map_err(|_| "Couldn't get cube")?;

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

fn main() -> Result<(), &'static str> {
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

    let socket =
        UdpSocket::bind(("0.0.0.0", MAIN_PORT)).map_err(|_| "Couldn't create UDP socket")?;
    let mut buf = [0; BUF_SIZE];

    'outer_loop: loop {
        println!("Waiting for organizer...");

        // wait for organizer ping / start
        let organizer = loop {
            let (len, addr) = socket
                .recv_from(&mut buf)
                .map_err(|_| "Couldn't recieve organizer ping")?;

            match buf[..len].try_into() {
                Ok(Command::Start) => break addr,
                Ok(Command::Ping) => {
                    socket
                        .send_to(
                            &[TryInto::<u8>::try_into(HostInfo {
                                host_type: HostType::Client {
                                    calibrated: cached_calibration.is_some(),
                                },
                                host_state: HostState::Idle,
                            })
                            .unwrap()],
                            addr,
                        )
                        .map_err(|_| "Couldn't reply with status")?;
                }
                _ => continue,
            }
        };

        let mut cam = VideoCapture::new(args.camera_index as i32, videoio::CAP_ANY)
            .map_err(|_| "Couldn't create camera instance")?;

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
        socket
            .send_to(&config.to_connection_request(pos).unwrap(), config.server)
            .map_err(|_| "Couldn't connect to server")?;

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
) -> Result<(), &'static str> {
    let mut draw = Mat::default();
    let mut aruco = Aruco::new(config.cube)?;

    if gui {
        highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)
            .map_err(|_| "Couldn't open window")?;
    }

    loop {
        let read_timeout = socket
            .read_timeout()
            .map_err(|_| "Couldn't get read timeout?!?!??!")?;

        socket
            .set_read_timeout(Some(Duration::from_millis(1)))
            .map_err(|_| "Couldn't set read timeout?!?!??!")?;

        match socket.recv_from(buf) {
            Ok((len, addr)) => match buf[..len].try_into() {
                Ok(Command::Stop) => break,

                Ok(Command::Ping) => {
                    socket
                        .send_to(
                            &[HostInfo {
                                host_type: HostType::Client { calibrated: true },
                                host_state: HostState::Running,
                            }
                            .try_into()
                            .unwrap()],
                            addr,
                        )
                        .map_err(|_| "Couldn't send status")?;
                }

                _ => (),
            },
            Err(e) if matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => (),
            Err(_) => Err("Error while receiving command")?,
        }

        socket
            .set_read_timeout(read_timeout)
            .map_err(|_| "Couldn't set read timeout?!?!??!")?;

        if gui && highgui::wait_key(10).map_err(|_| "Error while waiting for key")? == 113 {
            break;
        }

        // find & send x value
        cam.read(&mut frame).map_err(|_| "Couldn't read frame")?;

        frame
            .copy_to(&mut draw)
            .map_err(|_| "Couldn't copy frame")?;

        if let Some(data) = aruco.detect(frame, Some(&mut draw))? {
            socket
                .send_to(
                    &Into::<Vec<u8>>::into(Command::ValueUpdate(data)),
                    config.server,
                )
                .map_err(|_| "Couldn't send value")?;
        }

        if gui {
            highgui::imshow("videocap", &draw).map_err(|_| "Couldn't show frame")?;
        }
    }

    if gui {
        highgui::destroy_all_windows().map_err(|_| "Couldn't close window")?;
    }

    socket
        .send_to(&[Command::DISCONNECT], config.server)
        .map_err(|_| "Couldn't send disconnect")?;
    Ok(())
}

fn get_config(
    buf: &mut [u8],
    organizer: &IpAddr,
    cam: &mut VideoCapture,
    mut frame: &mut Mat,
    cached_calibration: &Option<FullCameraInfo>,
) -> Result<(Config, Position), &'static str> {
    let mut s = TcpStream::connect((*organizer, ORGANIZER_STARTER_PORT))
        .map_err(|_| "Couldn't connect to organizer tcp")?;

    'image_loop: loop {
        'request_wait_loop: loop {
            s.read_exact(&mut buf[..1])
                .map_err(|_| "Couldn't get organizer tcp command")?;

            match buf[..1].try_into() {
                Ok(Command::RequestImage) => break 'request_wait_loop,
                Ok(Command::ImagesDone) => break 'image_loop,
                _ => (),
            }
        }

        cam.read(&mut frame).map_err(|_| "Couldn't read frame")?;

        let mut image_buffer = core::Vector::new();
        opencv::imgcodecs::imencode(".jpg", frame, &mut image_buffer, &core::Vector::new())
            .map_err(|_| "Couldn't encode frame")?;

        let total = image_buffer.len() as u64;
        s.write_all(&total.to_be_bytes())
            .map_err(|_| "Couldn't send image len")?;
        s.write_all(image_buffer.as_slice())
            .map_err(|_| "Couldn't send image")?;
    }

    if let Some(c) = cached_calibration {
        s.write_all(c.horizontal_fov.to_be_bytes().as_slice())
            .map_err(|_| "Couldn't send fov")?;
    }

    // recieve camera info and server ip
    Config::from_organizer(&mut s, cached_calibration)
}

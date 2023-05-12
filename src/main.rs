mod aruco;
mod track;
mod util;

use aruco::Aruco;
use camloc_common::{
    calibration::FullCameraInfo,
    hosts::{
        constants::{MAIN_PORT, ORGANIZER_STARTER_PORT},
        ClientStatus, Command, HostStatus,
    },
};
use opencv::{
    core, highgui,
    prelude::*,
    videoio::{self, VideoCapture},
};
use std::{
    fs::File,
    io::{Read, Write},
    net::{IpAddr, SocketAddr, TcpStream, UdpSocket},
    time::Duration,
};
use track::Tracking;

use crate::aruco::detect;

const BUF_SIZE: usize = 2048;

struct Config {
    calibration: FullCameraInfo,
    server: SocketAddr,
    rotation: f64,
    x: f64,
    y: f64,
}

impl Config {
    fn to_connection_request(&self) -> Option<[u8; 33]> {
        let cmd = Into::<u8>::into(Command::Connect).to_be_bytes();
        let x = self.x.to_be_bytes();
        let y = self.y.to_be_bytes();
        let r = self.rotation.to_be_bytes();
        let f = self.calibration.horizontal_fov.to_be_bytes();

        [
            cmd.as_slice(),
            x.as_slice(),
            y.as_slice(),
            r.as_slice(),
            f.as_slice(),
        ]
        .concat()
        .try_into()
        .ok()
    }

    fn from_organizer(
        r: &mut impl Read,
        cached_calibration: &Option<FullCameraInfo>,
    ) -> Result<Self, &'static str> {
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

        Ok(Self {
            x,
            y,
            rotation,
            calibration,
            server,
        })
    }
}

fn main() -> Result<(), &'static str> {
    let args = {
        use clap::Parser;

        /// The camloc client
        #[derive(Parser)]
        struct Args {
            #[arg(short, long, default_value_t = 0u16)]
            camera_index: u16,

            /// Cache file
            #[arg(long, default_value = ".calib")]
            calibration_cache: String,
        }

        Args::parse()
    };

    let cached_calibration = if let Ok(mut f) = File::open(&args.calibration_cache) {
        println!("Found calibration file");
        FullCameraInfo::from_be_bytes(&mut f).ok()
    } else {
        None
    };

    let mut frame = Mat::default();
    let mut aruco = Aruco::new(2)?;
    let mut tracker = Tracking::new()?;

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

            if len != 1 {
                continue;
            }

            if buf[0] == Command::Start.into() {
                break addr;
            } else if buf[0] == Command::Ping.into() {
                socket
                    .send_to(
                        &[HostStatus::Client {
                            status: ClientStatus::Idle,
                            calibrated: cached_calibration.is_some(),
                        }
                        .try_into()
                        .unwrap()],
                        addr,
                    )
                    .map_err(|_| "Couldn't reply with status")?;
            }
        };

        let mut cam = VideoCapture::new(args.camera_index as i32, videoio::CAP_ANY)
            .map_err(|_| "Couldn't create camera instance")?;

        // recieve camera info and server ip
        let config = match get_config(
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
            .send_to(&config.to_connection_request().unwrap(), config.server)
            .map_err(|_| "Couldn't connect to server")?;

        inner_loop(
            &socket,
            &mut cam,
            &mut tracker,
            &mut aruco,
            config,
            &mut buf,
            &mut frame,
        )?;
    }
}

fn inner_loop(
    socket: &UdpSocket,
    cam: &mut VideoCapture,
    tracker: &mut Tracking,
    aruco: &mut Aruco,
    config: Config,
    buf: &mut [u8],
    mut frame: &mut Mat,
) -> Result<(), &'static str> {
    let mut draw = Mat::default();
    let mut has_object = false;

    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)
        .map_err(|_| "Couldn't open window")?;

    loop {
        let read_timeout = socket
            .read_timeout()
            .map_err(|_| "Couldn't get read timeout?!?!??!")?;

        socket
            .set_read_timeout(Some(Duration::from_millis(1)))
            .map_err(|_| "Couldn't set read timeout?!?!??!")?;

        if let Ok((1, addr)) = socket.recv_from(buf) {
            match TryInto::<Command>::try_into(buf[0]) {
                Ok(Command::Stop) => break,

                Ok(Command::Ping) => {
                    socket
                        .send_to(
                            &[HostStatus::Client {
                                status: ClientStatus::Running,
                                calibrated: true,
                            }
                            .try_into()
                            .unwrap()],
                            addr,
                        )
                        .map_err(|_| "Couldn't send status")?;
                }

                _ => (),
            }
        }

        socket
            .set_read_timeout(read_timeout)
            .map_err(|_| "Couldn't set read timeout?!?!??!")?;

        if highgui::wait_key(10).map_err(|_| "Error while waiting for key")? == 113 {
            break;
        }

        // find & send x value
        cam.read(&mut frame).map_err(|_| "Couldn't read frame")?;

        frame
            .copy_to(&mut draw)
            .map_err(|_| "Couldn't copy frame")?;

        let x = detect(frame, Some(&mut draw), &mut has_object, aruco, tracker)?;

        highgui::imshow("videocap", &draw).map_err(|_| "Couldn't show frame")?;

        socket
            .send_to(
                &[&[Command::ValueUpdate.into()], x.to_be_bytes().as_slice()].concat(),
                config.server,
            )
            .map_err(|_| "Couldn't send value")?;
    }

    highgui::destroy_all_windows().map_err(|_| "Couldn't close window")?;

    Ok(())
}

fn get_config(
    buf: &mut [u8],
    organizer: &IpAddr,
    cam: &mut VideoCapture,
    mut frame: &mut Mat,
    cached_calibration: &Option<FullCameraInfo>,
) -> Result<Config, &'static str> {
    let mut s = TcpStream::connect((*organizer, ORGANIZER_STARTER_PORT))
        .map_err(|_| "Couldn't connect to organizer tcp")?;

    'image_loop: loop {
        'request_wait_loop: loop {
            s.read_exact(&mut buf[..1])
                .map_err(|_| "Couldn't get organizer tcp command")?;

            if buf[0] == Command::RequestImage.into() {
                break 'request_wait_loop;
            } else if buf[0] == Command::ImagesDone.into() {
                break 'image_loop;
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

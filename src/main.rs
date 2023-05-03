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
use opencv::{core, highgui, prelude::*, videoio};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream, UdpSocket},
    time::Duration,
};
use track::Tracking;

use crate::aruco::detect;

const BUF_SIZE: usize = 2048;

struct Config {
    calibration: Option<FullCameraInfo>,
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
        let f = if let Some(c) = &self.calibration {
            c.horizontal_fov.to_be_bytes()
        } else {
            return None;
        };

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
        calibrated: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
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

        let calibration = if calibrated {
            Some(FullCameraInfo::from_be_bytes(r)?)
        } else {
            None
        };

        Ok(Self {
            calibration,
            rotation,
            server,
            x,
            y,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut frame = Mat::default();
    let mut draw = Mat::default();
    let mut aruco = Aruco::new(2)?;
    let mut tracker = Tracking::new()?;
    let mut has_object = false;

    let socket = UdpSocket::bind(("0.0.0.0", MAIN_PORT))?;
    let mut buf = [0; BUF_SIZE];

    loop {
        println!("waiting for connections...");

        // wait for organizer ping
        let organizer = loop {
            let (len, org) = socket.recv_from(&mut buf)?;
            if len == 1 && buf[0] == Command::Ping.into() {
                socket.send_to(
                    &[HostStatus::Client {
                        status: ClientStatus::Idle,
                        calibrated: false,
                    }
                    .try_into()
                    .unwrap()],
                    org,
                )?;
                break org;
            }
        };

        // wait for organizer start
        loop {
            let (len, addr) = socket.recv_from(&mut buf)?;
            if (addr == organizer) && (len == 1) && (buf[0] == Command::Start.into()) {
                break;
            }
        }

        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
        if !videoio::VideoCapture::is_opened(&cam)? {
            return Err("camera index not found!".into());
        }

        let mut s = TcpStream::connect((organizer.ip(), ORGANIZER_STARTER_PORT))
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

            cam.read(&mut frame)?;

            let mut image_buffer = core::Vector::new();
            opencv::imgcodecs::imencode(".jpg", &frame, &mut image_buffer, &core::Vector::new())?;

            let total = image_buffer.len() as u64;
            s.write_all(&total.to_be_bytes())
                .map_err(|_| "Couldn't send image len")?;
            s.write_all(image_buffer.as_slice())
                .map_err(|_| "Couldn't send image")?;
        }

        // recieve camera info and server ip
        let config = Config::from_organizer(&mut s, false)?;
        drop(s);

        // connect to server
        socket.send_to(&config.to_connection_request().unwrap(), config.server)?;
        socket.set_read_timeout(Some(Duration::from_millis(1)))?;

        highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
        loop {
            // Ok(1): one-byte recieved message
            // Err(): timeout
            if let Ok(1) = socket.recv(&mut buf) {
                match TryInto::<Command>::try_into(buf[0]) {
                    Ok(Command::Stop) => break,

                    Ok(Command::Ping) => {
                        socket.send_to(
                            &[HostStatus::Client {
                                status: ClientStatus::Running,
                                calibrated: true,
                            }
                            .try_into()
                            .unwrap()],
                            organizer,
                        )?;
                    }

                    _ => (),
                }
            }

            if highgui::wait_key(10)? == 113 {
                break;
            }

            // find & send x value
            cam.read(&mut frame)?;
            let x = detect(
                &mut frame,
                Some(&mut draw),
                &mut has_object,
                &mut aruco,
                &mut tracker,
            )?;

            highgui::imshow("videocap", &draw)?;
            socket.send_to(
                &[&[Command::ValueUpdate.into()], x.to_be_bytes().as_slice()].concat(),
                config.server,
            )?;
        }
        highgui::destroy_all_windows()?;
    }
}

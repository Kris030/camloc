mod aruco;
mod track;
mod util;

use std::{net::{SocketAddr, UdpSocket}, time::Duration};
use opencv::{core, highgui, prelude::*, videoio};
use camloc_common::hosts::{Command, constants::{MAIN_PORT, MAX_MESSAGE_LENGTH}, HostStatus, ClientStatus};
use track::Tracking;
use aruco::Aruco;

use crate::aruco::detect;

const BUF_SIZE: usize = 6 * 8;

struct Config {
    server: SocketAddr,
    rotation: f64,
    fov: f64,
    x: f64,
    y: f64,
}

impl Config {
    fn to_be_bytes(&self) -> Vec<u8> {
        [
            Into::<u8>::into(Command::Connect).to_be_bytes().as_slice(),
            self.x.to_be_bytes().as_slice(),
            self.y.to_be_bytes().as_slice(),
            self.rotation.to_be_bytes().as_slice(),
            self.fov.to_be_bytes().as_slice(),
        ].concat()
    }

    fn from_buffer(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let ip = String::from_utf8(buf[34..].to_vec())?;
        Ok(Self {
            x: f64::from_be_bytes(buf[0..7].try_into()?),
            y: f64::from_be_bytes(buf[8..15].try_into()?),
            rotation: f64::from_be_bytes(buf[16..23].try_into()?),
            fov: f64::from_be_bytes(buf[24..31].try_into()?),
            server: SocketAddr::new(ip.parse()?, MAIN_PORT),
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;

    let mut frame = Mat::default();
    let mut draw = Mat::default();
    let mut aruco = Aruco::new(2)?;
    let mut tracker = Tracking::new()?;
    let mut has_object = false;

    let socket = UdpSocket::bind(("0.0.0.0", MAIN_PORT))?;
    let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

    loop {
        println!("waiting for connections...");
        
        // wait for organizer ping
        let organizer = loop {
            let (len, org) = socket.recv_from(&mut buf)?;
            if len == 1 && buf[0] == Command::Ping.into() {
                socket.send_to(&[
                    HostStatus::Client {
                        status: ClientStatus::Idle,
                        calibrated: false,
                    }.try_into().unwrap()], org
                )?;
                break org;
            }
        };

        // wait for organizer start
        loop {
            let len = socket.recv(&mut buf)?;
            if len == 1 && buf[0] == Command::Connect.into() {
                break;
            }
        }

        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
        if !videoio::VideoCapture::is_opened(&cam)? {
            return Err("camera index not found!".into());
        }

        'image_loop: loop {
            cam.read(&mut frame)?;

            // TODO: simplify
            let mut image_buffer = core::Vector::new();
            opencv::imgcodecs::imencode(
                ".jpg",
                &frame,
                &mut image_buffer,
                &core::Vector::new(),
            )?;

            let total = image_buffer.len();
            let image_buffer = [
                (total as u64).to_be_bytes().as_slice(),
                image_buffer.as_slice(),
            ].concat();

            let total = total + std::mem::size_of::<u64>();

            let image_buffer = &image_buffer[..];
            let mut done = 0;

            while done != total {                
                let to_send = (total - done)
                    .min(MAX_MESSAGE_LENGTH);

                socket.send_to(
                    &image_buffer[done..(done + to_send)],
                    organizer
                )?;

                done += to_send;
            }

            'request_wait_loop: loop {
                let (len, addr) = socket.recv_from(&mut buf)?;
                if addr != organizer && len != 1 {
                    continue;
                }

                if buf[0] == Command::RequestImage.into() {
                    break 'request_wait_loop
                } else if buf[0] == Command::ImagesDone.into() {
                    break 'image_loop;
                }

            }
        }

        // recieve camera info and server ip
        socket.recv(&mut buf)?;
        let config = Config::from_buffer(&buf)?;

        // connect to server
        socket.send_to(&config.to_be_bytes(), config.server)?;
        socket.set_read_timeout(Some(Duration::from_millis(1)))?;

        loop {
            // Ok(1): one-byte recieved message
            // Err(): timeout
            if let Ok(1) = socket.recv(&mut buf) {
                match TryInto::<Command>::try_into(buf[0]) {
                    Ok(Command::Stop) => break,
    
                    Ok(Command::Ping) => {
                        socket.send_to(&[HostStatus::Client {
                            status: ClientStatus::Running, calibrated: true,
                        }.try_into().unwrap()], organizer)?;
                    },
    
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
            socket.send_to(&x.to_be_bytes(), config.server)?;
        }
    }
}

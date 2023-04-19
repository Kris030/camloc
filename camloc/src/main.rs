mod aruco;
mod track;
mod util;

use aruco::Aruco;
use opencv::{highgui, prelude::*, videoio};
use std::{
    net::{SocketAddr, UdpSocket},
    time::Duration,
};
use track::Tracking;

use crate::aruco::detect;

const PING: u8 = 0x0b;
const PONG: u8 = 0xcf;
const START: u8 = 0x60;
const STOP: u8 = 0xcd;
const CONNECT: u8 = 0xcc;
const BUF_SIZE: usize = 6 * 8;

struct Config {
    x: f64,
    y: f64,
    rotation: f64,
    fov: f64,
    server: SocketAddr,
}

impl Config {
    fn to_be_bytes(&self) -> Vec<u8> {
        vec![
            CONNECT.to_be_bytes().as_slice(),
            self.x.to_be_bytes().as_slice(),
            self.y.to_be_bytes().as_slice(),
            self.rotation.to_be_bytes().as_slice(),
            self.fov.to_be_bytes().as_slice(),
        ]
        .concat()
    }

    fn from_buffer(buf: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let ip = String::from_utf8(buf[34..].to_vec())?;
        Ok(Self {
            x: f64::from_be_bytes(buf[0..7].try_into()?),
            y: f64::from_be_bytes(buf[8..15].try_into()?),
            rotation: f64::from_be_bytes(buf[16..23].try_into()?),
            fov: f64::from_be_bytes(buf[24..31].try_into()?),
            server: SocketAddr::new(ip.parse()?, 1234),
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

    let socket = UdpSocket::bind("0.0.0.0:1111")?;
    let mut buf: [u8; BUF_SIZE] = [(); BUF_SIZE].map(|_| 0);

    loop {
        let mut organizer: SocketAddr;
        println!("waiting for connections...");

        // wait for organizer ping
        loop {
            (_, organizer) = socket.recv_from(&mut buf)?;
            if buf[0] == PING {
                socket.send_to(&[PONG], organizer)?;
                break;
            }
        }

        // wait for organizer start
        loop {
            socket.recv(&mut buf)?;
            if buf[0] == START {
                break;
            }
        }

        let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
        if !videoio::VideoCapture::is_opened(&cam)? {
            panic!("camera index not found!");
        }
        cam.read(&mut frame)?;

        let mut image_buffer = opencv::core::Vector::<u8>::new();
        opencv::imgcodecs::imencode(
            ".jpg",
            &frame,
            &mut image_buffer,
            &opencv::core::Vector::new(),
        )?;
        socket.send_to(&image_buffer.as_slice(), organizer)?;

        // recieve camera info and server ip
        socket.recv(&mut buf)?;
        let config = Config::from_buffer(&buf)?;

        // connect to server
        socket.send_to(&config.to_be_bytes(), config.server)?;
        socket.set_read_timeout(Some(Duration::from_millis(1)))?;

        loop {
            socket.recv(&mut buf)?;
            match buf[0] {
                STOP => break,

                PING => {
                    socket.send_to(&[PONG], organizer)?;
                }

                _ => {
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
    }
}

mod aruco;
mod track;
mod util;

#[allow(unused)]
use aruco::Aruco;
use opencv::{highgui, prelude::*, videoio};
// use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
// use track::Tracking;

const PING: u8 = 0x0b;
const PONG: u8 = 0xca;
const START: u8 = 0x60;

#[allow(unused)]
struct Config {
    x: f64,
    y: f64,
    rotation: f64,
    fov: f64,
}

// longest case -> x: f64, y: f64, rotation: f64, fov: f64
const BUF_SIZE: usize = 4 * 8;

#[allow(unused)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    highgui::named_window("videocap", highgui::WINDOW_AUTOSIZE)?;
    let mut cam = videoio::VideoCapture::new(0, videoio::CAP_ANY)?;
    if !videoio::VideoCapture::is_opened(&cam)? {
        panic!("camera index not found!");
    }
    let mut frame = Mat::default();

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
                cam.read(&mut frame)?;
                socket.send_to(frame.data_bytes()?, organizer)?;
                break;
            }
        }

        // wait for organizer start
        loop {
            socket.recv(&mut buf)?;
            let config = Config {
                x: f64::from_be_bytes(buf[0..7].try_into()?),
                y: f64::from_be_bytes(buf[8..15].try_into()?),
                rotation: f64::from_be_bytes(buf[16..23].try_into()?),
                fov: f64::from_be_bytes(buf[24..31].try_into()?),
            };
        }
    }

    // let mut frame = Mat::default();
    // let mut draw = Mat::default();

    // let mut aruco = Aruco::new(2)?;
    // let mut tracker = Tracking::new()?;
    // let mut has_object = false;

    // loop {
    //     println!("Waiting for connections on {}", port);
    //     let (mut tcp_stream, addr) = listener.accept()?;
    //     println!("Connection received from {:?}", addr);

    //     while highgui::wait_key(10)? != 113 {
    //         cam.read(&mut frame)?;
    //         if frame.size()?.width < 1 {
    //             continue;
    //         }
    //         draw = frame.clone();
    //         let mut final_x = f64::NAN;
    //         // tracking logic
    //         if !has_object {
    //             if let Some(x) =
    //                 aruco.detect(&mut frame, Some(&mut tracker.rect), Some(&mut draw))?
    //             {
    //                 final_x = x;
    //                 has_object = true;
    //                 tracker.init(&frame);
    //             }
    //         } else {
    //             if let Some(x) = tracker.track(&frame, Some(&mut draw))? {
    //                 final_x = x;
    //             } else {
    //                 has_object = false;
    //             }
    //         }

    //         highgui::imshow("videocap", &draw)?;
    //         if tcp_stream.write_all(&final_x.to_be_bytes()).is_err() {
    //             break;
    //         }
    //     }
    // }

    Ok(())
}

mod scanning;
mod utils;

use camloc_common::{
    cv::{self, display_image},
    get_from_stdin,
    hosts::constants::MAIN_PORT,
    hosts::{constants::ORGANIZER_STARTER_PORT, Command, HostInfo, HostState, HostType},
    position::{calc_posotion_in_square_distance, get_camera_distance_in_square, Position},
};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use opencv::{core, imgcodecs, prelude::*};
use scanning::IPV4AddressTemplate;
use std::{
    io::{Read, Write},
    mem::size_of,
    net::{IpAddr, TcpListener, UdpSocket},
    time::{Duration, Instant},
};

pub(crate) struct Host {
    pub info: HostInfo,
    pub ip: IpAddr,
}

fn get_own_ip() -> Result<Addr, String> {
    let nis = NetworkInterface::show().map_err(|_| "Couldn't get network interfaces")?;
    let mut rnis = vec![];
    println!("Interfaces and addresses:");
    let mut ai = 0;
    for n in &nis {
        println!("{}", n.name);
        for a in &n.addr {
            let ip = a.ip();
            if !ip.is_ipv4() {
                continue;
            }

            rnis.push(*a);
            println!("{ai:<3}{ip}");
            ai += 1;
        }
    }

    let ai: usize = get_from_stdin("\nEnter ip index: ")?;

    rnis.get(ai).copied().ok_or("Invalid index".to_string())
}

enum SetupType {
    Square { side_length: f64 },
    Free,
}

impl SetupType {
    fn select_camera_position(&self, fov: f64) -> Result<Position, &'static str> {
        println!("Enter camera position");
        Ok(match self {
            SetupType::Square { side_length } => calc_posotion_in_square_distance(
                get_from_stdin("  Camera index: ")?,
                get_camera_distance_in_square(*side_length, fov),
            ),
            SetupType::Free => Position::new(
                get_from_stdin("  x: ")?,
                get_from_stdin("  y: ")?,
                get_from_stdin::<f64>("  rotation (degrees): ")?.to_radians(),
            ),
        })
    }
}

fn get_setup_type() -> Result<SetupType, &'static str> {
    match get_from_stdin("Select setup type square (0) / free (1): ")? {
        0 => Ok(SetupType::Square {
            side_length: get_from_stdin("Enter side length: ")?,
        }),

        1 => Ok(SetupType::Free),
        _ => Err("Invalid index"),
    }
}

fn main() -> Result<(), String> {
    let args = {
        use clap::Parser;

        /// The camloc organizer
        #[derive(Parser)]
        struct Args {
            /// The arcuco ids on the cube (counterclockwise)
            #[arg(short, long, required = true, num_args = 4)]
            cube: Vec<u8>,
        }

        Args::parse()
    };
    let setup_type = get_setup_type()?;

    let own_ip = get_own_ip()?;
    println!("Selected {}\n", own_ip.ip());

    let hosts = &mut vec![];
    let sock = UdpSocket::bind(("0.0.0.0", 0)).map_err(|_| "Couldn't create socket")?;
    let server_sock = TcpListener::bind(("0.0.0.0", ORGANIZER_STARTER_PORT))
        .map_err(|_| "Couldn't create socket")?;

    let mut organizer = Organizer {
        buffer: &mut [0; 2048],
        server_sock,
        setup_type,
        cube: args.cube.try_into().unwrap(),
        hosts,
        sock,
    };

    loop {
        organizer.scan(own_ip)?;
        organizer.handle_commands()?;
    }
}
struct Organizer<'a, 'b, const BUFFER_SIZE: usize> {
    buffer: &'a mut [u8; BUFFER_SIZE],
    hosts: &'b mut Vec<Host>,
    server_sock: TcpListener,
    setup_type: SetupType,
    cube: [u8; 4],
    sock: UdpSocket,
}

impl<const BUFFER_SIZE: usize> Organizer<'_, '_, BUFFER_SIZE> {
    fn handle_commands(&mut self) -> Result<(), String> {
        let Ok(ind) = get_from_stdin::<usize>("Enter command: start (0) / stop (1): ") else {
            return Ok(())
        };
        println!();
        match ind {
            0 => {
                if let Err(e) = self.start_host() {
                    println!("Couldn't start client because: {e}");
                }
            }

            1 => {
                if let Err(e) = self.stop_host() {
                    println!("Couldn't stop client because: {e}");
                }
            }
            _ => (),
        }
        println!();
        Ok(())
    }

    fn start_host(&mut self) -> Result<(), String> {
        let server = match utils::get_server(&mut *self.hosts) {
            Ok(s) => s,
            Err(count) => {
                println!("{count} servers running, resolve first");
                return Ok(());
            }
        };
        let server_ip = server.ip.to_string();
        match server.info {
            HostInfo {
                host_type: HostType::Server,
                host_state: HostState::Idle,
            } => {
                let s: String =
                    get_from_stdin("Server isn't running, do you want to start it? (y) ")?;
                if !matches!(&s[..], "" | "y" | "Y") {
                    return Ok(());
                }

                self.sock
                    .send_to(
                        &Into::<Vec<u8>>::into(Command::StartServer { cube: self.cube }),
                        (server.ip, MAIN_PORT),
                    )
                    .map_err(|_| "Couldn't start server")?;

                return Ok(());
            }
            HostInfo {
                host_type: HostType::Server,
                host_state: _,
            } => (),
            _ => unreachable!(),
        }

        let options = utils::print_hosts(self.hosts, |s| {
            matches!(
                s,
                HostInfo {
                    host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                    host_state: HostState::Idle
                }
            )
        });
        if options.is_empty() {
            println!("No clients found");
            return Ok(());
        }

        let selected: usize = get_from_stdin("\nSelect client to start: ")?;
        let host_index = *options.get(selected).ok_or("No such index")?;
        let addr = ((self.hosts[host_index]).ip, MAIN_PORT);

        self.sock
            .send_to(&[Command::START], addr)
            .map_err(|_| "Couldn't send client start")?;

        // wait for connection on the serversocket
        let mut s = loop {
            let (s, a) = self
                .server_sock
                .accept()
                .map_err(|_| "Couldn't accept connection")?;
            if addr.0 == a.ip() {
                break s;
            }
        };

        let uncalibrated = match self.hosts[host_index].info {
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

        let mut uncalibrated = if uncalibrated {
            println!("Starting calibration");
            let width: u8 = get_from_stdin("  Charuco board width: ")?;
            let height: u8 = get_from_stdin("  Charuco board height: ")?;

            Some((
                cv::generate_board(width, height).map_err(|_| "Couldn't create charuco board")?,
                vec![],
            ))
        } else {
            println!("Starting image stream");
            None
        };

        loop {
            s.write_all(&[Command::REQUEST_IMAGE])
                .map_err(|_| "Couldn't request image")?;

            let img = self.get_image(&mut s)?;

            if let Some((board, imgs)) = &mut uncalibrated {
                let detection =
                    cv::find_board(&img, board, false).map_err(|_| "Couldn't find board")?;

                if let Some(fb) = detection {
                    let mut drawn_boards = img.clone();
                    cv::draw_board(&mut drawn_boards, &fb)
                        .map_err(|_| "Couldn't draw detected boards")?;
                    display_image(&drawn_boards, "recieved", true)
                        .map_err(|_| "Couldn't display image")?;

                    let keep =
                        get_from_stdin::<String>("  Keep image? (y) ")?.to_lowercase() == "y";
                    if keep {
                        imgs.push(img);
                    }

                    if imgs.is_empty() {
                        println!("  You can't calibrate with no images");
                        continue;
                    }
                } else {
                    display_image(&img, "recieved", true).map_err(|_| "Couldn't display image")?;

                    print!("  Board not found\n  ");
                }
            } else {
                display_image(&img, "recieved", true).map_err(|_| "Couldn't display image")?;
            }

            let more = get_from_stdin::<String>("  Continue? (y) ")?.to_lowercase() == "y";
            if !more {
                let _ = opencv::highgui::destroy_window("recieved");
                break;
            }
        }
        s.write_all(&[Command::IMAGES_DONE])
            .map_err(|_| "Couldn't send images done")?;

        let ip_bytes = server_ip.as_bytes();
        let ip_len = ip_bytes.len() as u16;

        let (pos, calib) = if let Some((board, images)) = &uncalibrated {
            let calib = cv::calibrate(
                board,
                images,
                images[0].size().map_err(|_| "Couldn't get image size??")?,
            )
            .map_err(|_| "Couldn't calibrate")?;

            let pos = self
                .setup_type
                .select_camera_position(calib.horizontal_fov)?;

            (pos, Some(calib))
        } else {
            let fov = &mut self.buffer[..size_of::<f64>()];
            s.read_exact(fov)
                .map_err(|_| "Couldn't get fov from calibrated client")?;
            let fov = f64::from_be_bytes(fov.try_into().unwrap());
            (self.setup_type.select_camera_position(fov)?, None)
        };

        s.write_all(pos.x.to_be_bytes().as_slice())
            .map_err(|_| "Couldn't write x")?;
        s.write_all(pos.y.to_be_bytes().as_slice())
            .map_err(|_| "Couldn't write y")?;
        s.write_all(pos.rotation.to_be_bytes().as_slice())
            .map_err(|_| "Couldn't write rotation")?;
        s.write_all(ip_len.to_be_bytes().as_slice())
            .map_err(|_| "Couldn't write ip len")?;
        s.write_all(ip_bytes)
            .map_err(|_| "Couldn't write ip bytes")?;

        if let Some(calib) = calib {
            s.write_all(calib.to_be_bytes().as_slice())
                .map_err(|_| "Couldn't write calibration")?;
        }

        s.write_all(&self.cube.map(u8::to_be))
            .map_err(|_| "Couldn't write cube info")?;

        match &mut self.hosts[host_index].info {
            HostInfo {
                host_type: HostType::ConfiglessClient,
                host_state,
            } => *host_state = HostState::Running,
            HostInfo {
                host_type: HostType::Client { calibrated },
                host_state,
            } => {
                *host_state = HostState::Running;
                *calibrated = true;
            }
            HostInfo {
                host_type: HostType::Server,
                ..
            } => unreachable!(),
        };

        Ok(())
    }

    fn stop_host(&mut self) -> Result<(), String> {
        let options = utils::print_hosts(self.hosts, |s| {
            matches!(
                s,
                HostInfo {
                    host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                    host_state: HostState::Running,
                }
            )
        });

        let selected: usize = get_from_stdin("\nSelect client to start: ")?;
        let host = options[selected];

        let addr = (self.hosts[host].ip, MAIN_PORT);
        self.sock
            .send_to(&[Command::STOP], addr)
            .map_err(|_| "Couldn't send client start")?;

        self.hosts.remove(host);

        Ok(())
    }

    fn scan(&mut self, own_ip: Addr) -> Result<(), &'static str> {
        println!("Scanning...\n");
        let IpAddr::V4(ip) = own_ip.ip() else {
            unreachable!()
        };

        let set_broadcast = self.sock.set_broadcast(true).is_ok();

        self.sock
            .set_read_timeout(Some(TIMEOUT_DURATION))
            .map_err(|_| "Couldn't set timeout")?;

        match own_ip.broadcast() {
            Some(broadcast) if set_broadcast && !ip.is_loopback() => {
                self.scan_with_broadcast(broadcast)?;
                assert!(
                    self.sock.set_broadcast(false).is_ok(),
                    "Couldn't unset broadcast?"
                );
            }
            _ => {
                let netmask = own_ip.netmask().expect("No netmask");
                let netmask = if let IpAddr::V4(n) = netmask {
                    n
                } else {
                    unreachable!()
                };
                self.scan_with_template(IPV4AddressTemplate::from_netmask(
                    ip,
                    scanning::get_netmask_bits(netmask) as usize,
                    scanning::TemplateMember::Fixed(MAIN_PORT),
                ))?;
            }
        }

        Ok(())
    }

    #[allow(unused, clippy::ptr_arg)]
    fn scan_with_template(&mut self, template: IPV4AddressTemplate) -> Result<(), &'static str> {
        todo!()
    }

    fn scan_with_broadcast(&mut self, broadcast: IpAddr) -> Result<(), &'static str> {
        self.sock
            .send_to(&[Command::PING], (broadcast, MAIN_PORT))
            .map_err(|_| "Couldn't send ping")?;

        let till = Instant::now() + WAIT_DURATION;

        let mut hit_hosts = vec![false; self.hosts.len()];

        'loopy: while Instant::now() < till {
            let Ok((1, addr)) = self.sock.recv_from(self.buffer) else {
                continue;
            };

            let ip = addr.ip();
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

    fn get_image(&mut self, r: &mut impl Read) -> Result<Mat, &'static str> {
        r.read_exact(&mut self.buffer[..size_of::<u64>()])
            .map_err(|_| "Couldn't read image len")?;
        let len = u64::from_be_bytes(self.buffer[..size_of::<u64>()].try_into().unwrap()) as usize;

        let mut buffer = core::Vector::from_elem(0, len);

        r.read_exact(&mut buffer.as_mut_slice()[..len])
            .map_err(|_| "Couldn't read image")?;

        imgcodecs::imdecode(&buffer, imgcodecs::IMREAD_COLOR).map_err(|_| "Couldn't decode image")
    }
}

const TIMEOUT_DURATION: Duration = Duration::from_millis(500);
const WAIT_DURATION: Duration = Duration::from_millis(TIMEOUT_DURATION.as_millis() as u64 * 4);

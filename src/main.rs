mod scanning;
mod utils;

use anyhow::{anyhow, Result};
use camloc_common::{
    choice,
    cv::{self, display_image},
    get_from_stdin,
    hosts::constants::MAIN_PORT,
    hosts::{constants::ORGANIZER_STARTER_PORT, Command, HostInfo, HostState, HostType},
    position::{calc_position_in_square_distance, get_camera_distance_in_square},
    yes_no_choice, Position,
};
use network_interface::{Addr, NetworkInterface, NetworkInterfaceConfig};
use camloc_common::opencv::{self, core, imgcodecs, prelude::*};
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

fn get_own_ip() -> Result<Addr> {
    let nis = NetworkInterface::show()?;
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

    rnis.get(ai).copied().ok_or(anyhow!("Invalid index"))
}

enum SetupType {
    Square { side_length: f64 },
    Free,
}

impl SetupType {
    fn select_camera_position(&self, fov: f64) -> Result<Position> {
        println!("Enter camera position");
        Ok(match self {
            SetupType::Square { side_length } => calc_position_in_square_distance(
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

fn get_setup_type() -> Result<SetupType> {
    match choice(
        [("Square", true), ("Free", true)].into_iter(),
        Some("Select setup type: "),
        Some(1),
    )? {
        0 => Ok(SetupType::Square {
            side_length: get_from_stdin("Enter side length: ")?,
        }),

        1 => Ok(SetupType::Free),
        _ => Err(anyhow!("Invalid index")),
    }
}

fn main() -> Result<()> {
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
    let sock = UdpSocket::bind(("0.0.0.0", 0))?;
    let server_sock = TcpListener::bind(("0.0.0.0", ORGANIZER_STARTER_PORT))?;

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
    fn handle_commands(&mut self) -> Result<()> {
        let server = match utils::get_server(&mut *self.hosts) {
            Ok(s) => s,
            Err(count) => {
                println!("{count} servers running, resolve first");
                return Ok(());
            }
        };
        let server_ip = server.ip;
        if let HostState::Idle = server.info.host_state {
            if !yes_no_choice("Server isn't running, do you want to start it?", true) {
                return Ok(());
            }

            self.sock.send_to(
                &Into::<Vec<u8>>::into(Command::StartServer { cube: self.cube }),
                (server.ip, MAIN_PORT),
            )?;

            return Ok(());
        }

        #[derive(Debug, Clone, Copy)]
        enum OrganizerCommand {
            Start,
            Stop,
            List,
            Scan,
            Update,
            Quit,
        }
        use OrganizerCommand::*;
        const COMMANDS: [OrganizerCommand; 6] = [Start, Stop, List, Scan, Update, Quit];
        impl std::fmt::Display for OrganizerCommand {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{self:?}")
            }
        }

        let cmd = choice(
            COMMANDS.map(|c| (c, true)).into_iter(),
            Some("Choose action: "),
            None,
        );
        let Ok(cmd) = cmd.map(|i| COMMANDS[i]) else {
            return Ok(())
        };
        println!();
        match cmd {
            Start => {
                let server_ip = server_ip.to_string();
                if let Err(e) = self.start_host(&server_ip) {
                    println!("Couldn't start client because: {e}");
                }
            }

            Stop => {
                if let Err(e) = self.stop_host() {
                    println!("Couldn't stop client because: {e}");
                }
            }
            List => {
                for h in self.hosts.iter() {
                    println!("{h}");
                }
            }
            Scan => (),
            Update => 'update: {
                let options: Vec<(&Host, bool)> = self
                    .hosts
                    .iter()
                    .filter_map(|h| {
                        if matches!(
                            h.info,
                            HostInfo {
                                host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                                host_state: HostState::Idle
                            }
                        ) {
                            Some((h, true))
                        } else {
                            None
                        }
                    })
                    .collect();
                if options.is_empty() {
                    println!("No clients found");
                    break 'update;
                }

                let host_index = choice(
                    options.into_iter(),
                    Some("\nSelect client to update: "),
                    None,
                )?;

                let position = Position::new(
                    get_from_stdin("  x: ")?,
                    get_from_stdin("  y: ")?,
                    get_from_stdin::<f64>("  rotation (degrees): ")?.to_radians(),
                );
                let fov = if yes_no_choice("  Do you also want to change the fov?", false) {
                    Some(get_from_stdin::<f64>("  fov (degrees): ")?.to_radians())
                } else {
                    None
                };

                self.sock.send_to(
                    &Into::<Vec<u8>>::into(Command::InfoUpdate {
                        client_ip: &self.hosts[host_index].ip.to_string(),
                        position,
                        fov,
                    }),
                    (server_ip, MAIN_PORT),
                )?;
            }
            Quit => {
                println!("Quitting...");
                std::process::exit(0)
            }
        }
        println!();
        Ok(())
    }

    fn start_host(&mut self, server: &str) -> Result<()> {
        let options: Vec<(&Host, bool)> = self
            .hosts
            .iter()
            .map(|h| {
                (h, {
                    matches!(
                        h.info,
                        HostInfo {
                            host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                            host_state: HostState::Idle
                        }
                    )
                })
            })
            .collect();
        if options.is_empty() {
            println!("No clients found");
            return Ok(());
        }

        let host_index = choice(
            options.into_iter(),
            Some("\nSelect client to start: "),
            None,
        )?;
        let addr = ((self.hosts[host_index]).ip, MAIN_PORT);

        self.sock.send_to(&[Command::START], addr)?;

        // wait for connection on the serversocket
        let mut s = loop {
            let (s, a) = self.server_sock.accept()?;
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

            Some((cv::generate_board(width, height)?, vec![]))
        } else {
            println!("Starting image stream");
            None
        };

        loop {
            s.write_all(&[Command::REQUEST_IMAGE])?;

            let img = self.get_image(&mut s)?;

            if let Some((board, imgs)) = &mut uncalibrated {
                let detection = cv::find_board(&img, board, false)?;

                if let Some(fb) = detection {
                    let mut drawn_boards = img.clone();
                    cv::draw_board(&mut drawn_boards, &fb)?;
                    display_image(&drawn_boards, "recieved", true)?;

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
                    display_image(&img, "recieved", true)?;

                    print!("  Board not found\n  ");
                }
            } else {
                display_image(&img, "recieved", true)?;
            }

            let more = get_from_stdin::<String>("  Continue? (y) ")?.to_lowercase() == "y";
            if !more {
                let _ = opencv::highgui::destroy_window("recieved");
                break;
            }
        }
        s.write_all(&[Command::IMAGES_DONE])?;

        let ip_bytes = server.as_bytes();
        let ip_len = ip_bytes.len() as u16;

        let (pos, calib) = if let Some((board, images)) = &uncalibrated {
            let calib = cv::calibrate(board, images, images[0].size()?)?;

            let pos = self
                .setup_type
                .select_camera_position(calib.horizontal_fov)?;

            (pos, Some(calib))
        } else {
            let fov = &mut self.buffer[..size_of::<f64>()];
            s.read_exact(fov)?;
            let fov = f64::from_be_bytes(fov.try_into()?);
            (self.setup_type.select_camera_position(fov)?, None)
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

    fn stop_host(&mut self) -> Result<()> {
        let options = self.hosts.iter().map(|s| {
            (s, {
                matches!(
                    s.info,
                    HostInfo {
                        host_type: HostType::Client { .. } | HostType::ConfiglessClient,
                        host_state: HostState::Running,
                    }
                )
            })
        });

        let host = choice(options, Some("\nSelect client to start: "), None)?;

        let addr = (self.hosts[host].ip, MAIN_PORT);
        self.sock.send_to(&[Command::STOP], addr)?;

        self.hosts.remove(host);

        Ok(())
    }

    fn scan(&mut self, own_ip: Addr) -> Result<()> {
        println!("Scanning...\n");
        let IpAddr::V4(ip) = own_ip.ip() else {
            unreachable!()
        };

        let set_broadcast = self.sock.set_broadcast(true).is_ok();

        self.sock.set_read_timeout(Some(TIMEOUT_DURATION))?;

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
    fn scan_with_template(&mut self, template: IPV4AddressTemplate) -> Result<()> {
        todo!()
    }

    fn scan_with_broadcast(&mut self, broadcast: IpAddr) -> Result<()> {
        self.sock
            .send_to(&[Command::PING], (broadcast, MAIN_PORT))?;

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

    fn get_image(&mut self, r: &mut impl Read) -> Result<Mat> {
        r.read_exact(&mut self.buffer[..size_of::<u64>()])?;
        let len = u64::from_be_bytes(self.buffer[..size_of::<u64>()].try_into()?) as usize;

        let mut buffer = core::Vector::from_elem(0, len);

        r.read_exact(&mut buffer.as_mut_slice()[..len])?;

        Ok(imgcodecs::imdecode(&buffer, imgcodecs::IMREAD_COLOR)?)
    }
}

const TIMEOUT_DURATION: Duration = Duration::from_millis(500);
const WAIT_DURATION: Duration = Duration::from_millis(TIMEOUT_DURATION.as_millis() as u64 * 4);

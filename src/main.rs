mod scanning;
mod utils;

use camloc_common::{
    calibration::{self, display_image},
    get_from_stdin,
    hosts::constants::MAIN_PORT,
    hosts::{constants::ORGANIZER_STARTER_PORT, ClientStatus, Command, HostStatus, ServerStatus},
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
    pub status: HostStatus,
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
                get_from_stdin("  Camera index")?,
                get_camera_distance_in_square(*side_length, fov),
            ),
            SetupType::Free => Position::new(
                get_from_stdin("  x: ")?,
                get_from_stdin("  y: ")?,
                get_from_stdin("  rotation: ")?,
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
    sock: UdpSocket,
}

impl<const BUFFER_SIZE: usize> Organizer<'_, '_, BUFFER_SIZE> {
    fn handle_commands(&mut self) -> Result<(), String> {
        let get_from_stdin: usize = get_from_stdin("Enter command: start (0) / stop (1) client: ")?;
        println!();
        match get_from_stdin {
            0 => if let Err(e) = self.start_client() {
                println!("Couldn't start client because: {e}");
            },

            1 => if let Err(e) = self.stop_client() {
                println!("Couldn't stop client because: {e}");
            },
            _ => (),
        }
        println!();
        Ok(())
    }

    fn start_client(&mut self) -> Result<(), String> {
        let server = match utils::get_server(&mut *self.hosts) {
            Ok(s) => s,
            Err(count) => {
                println!("{count} servers running, resolve first");
                return Ok(());
            }
        };
        let server_ip = server.ip.to_string();

        let options = utils::print_hosts(self.hosts, |s| {
            matches!(
                s,
                HostStatus::Client {
                    status: ClientStatus::Idle,
                    ..
                } | HostStatus::ConfiglessClient(ClientStatus::Idle)
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
            .send_to(&[Command::Start.into()], addr)
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

        let uncalibrated = match self.hosts[host_index].status {
            HostStatus::Client { calibrated, .. } => !calibrated,
            HostStatus::ConfiglessClient(_) => return Ok(()),
            HostStatus::Server(_) => unreachable!(),
        };

        let mut uncalibrated = if uncalibrated {
            println!("Starting calibration");
            let width: u8 = get_from_stdin("  Charuco board width: ")?;
            let height: u8 = get_from_stdin("  Charuco board height: ")?;

            Some((
                calibration::generate_board(width, height)
                    .map_err(|_| "Couldn't create charuco board")?,
                vec![],
            ))
        } else {
            println!("Starting image stream");
            None
        };

        loop {
            s.write_all(&[Command::RequestImage.into()])
                .map_err(|_| "Couldn't request image")?;

            let img = self.get_image(&mut s)?;

            if let Some((board, imgs)) = &mut uncalibrated {
                let detection = calibration::find_board(&img, board, false)
                    .map_err(|_| "Couldn't find board")?;

                if let Some(fb) = detection {
                    let mut drawn_boards = img.clone();
                    calibration::draw_board(&mut drawn_boards, &fb)
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
        s.write_all(&[Command::ImagesDone.into()])
            .map_err(|_| "Couldn't send images done")?;

        let ip_bytes = server_ip.as_bytes();
        let ip_len = ip_bytes.len() as u16;

        let (pos, calib) = if let Some((board, imgs)) = &uncalibrated {
            let calib = calibration::calibrate(board, imgs).map_err(|_| "Couldn't calibrate")?;
            let pos = self
                .setup_type
                .select_camera_position(calib.horizontal_fov)?;

            (pos, Some(calib))
        } else {
            (self.setup_type.select_camera_position(f64::NAN)?, None)
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

        Ok(())
    }

    fn stop_client(&mut self) -> Result<(), String> {
        let options = utils::print_hosts(self.hosts, |s| {
            matches!(
                s,
                HostStatus::Client {
                    status: ClientStatus::Running,
                    ..
                } | HostStatus::ConfiglessClient(ClientStatus::Running)
            )
        });

        let selected: usize = get_from_stdin("\nSelect client to start: ")?;
        let host = options[selected];

        let addr = (self.hosts[host].ip, MAIN_PORT);
        self.sock
            .send_to(&[Command::Stop.into()], addr)
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
                self.scan_with_broadcast(broadcast)
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
                ))
            }
        }
    }

    #[allow(unused, clippy::ptr_arg)]
    fn scan_with_template(&mut self, template: IPV4AddressTemplate) -> Result<(), &'static str> {
        todo!()
    }

    fn scan_with_broadcast(&mut self, broadcast: IpAddr) -> Result<(), &'static str> {
        self.sock
            .send_to(&[Command::Ping.into()], (broadcast, MAIN_PORT))
            .map_err(|_| "Couldn't send ping")?;

        let till = Instant::now() + WAIT_DURATION;

        let mut hit_hosts = vec![false; self.hosts.len()];

        'loopy: while Instant::now() < till {
            let Ok((msg_len, addr)) = self.sock.recv_from(self.buffer) else {
                continue;
            };
            if msg_len != 1 {
                continue;
            }

            let ip = addr.ip();
            let Ok(status) = self.buffer[0].try_into() else {
                continue 'loopy;
            };

            let h: _ = self
                .hosts
                .iter_mut()
                .zip(hit_hosts.iter_mut())
                .find(|(h, _)| h.ip == ip);

            if let Some((h, hit)) = h {
                *hit = true;
                h.status = status;
            } else {
                self.hosts.push(Host { status, ip });
            }
        }

        for (h, hit) in self.hosts.iter_mut().zip(hit_hosts.iter()) {
            if *hit {
                continue;
            }
            h.status = match h.status {
                HostStatus::ConfiglessClient(_) => {
                    HostStatus::ConfiglessClient(ClientStatus::Unreachable)
                }
                HostStatus::Client { calibrated, .. } => HostStatus::Client {
                    status: ClientStatus::Unreachable,
                    calibrated,
                },
                HostStatus::Server(_) => HostStatus::Server(ServerStatus::Unreachable),
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

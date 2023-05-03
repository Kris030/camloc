mod scanning;
mod utils;

use camloc_common::{
    position::{Position, get_camera_distance_in_square, calc_posotion_in_square_distance},
    hosts::{HostStatus, ClientStatus, ServerStatus, Command},
    hosts::constants::{MAIN_PORT, MAX_MESSAGE_LENGTH},
    calibration::{self, display_image},
    get_from_stdin,
};
use network_interface::{NetworkInterfaceConfig, NetworkInterface, Addr};
use std::{net::{IpAddr, UdpSocket}, time::{Duration, Instant}, mem::size_of};
use opencv::{prelude::*, imgcodecs, core};
use scanning::IPV4AddressTemplate;

pub(crate) struct Host {
    pub status: HostStatus,
    pub ip: IpAddr,
}

fn get_own_ip() -> Result<Addr, String> {
    let nis = NetworkInterface::show()
        .map_err(|_| "Couldn't get network interfaces")?;
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

    rnis.get(ai)
        .copied()
        .ok_or("Invalid index".to_string())
}

enum SetupType {
    Square { side_length: f64 },
    Free,
}

impl SetupType {
    fn select_camera_position(&self, fov: f64) -> Result<Position, &'static str> {
        println!("Enter camera position");
        Ok(match self {
            SetupType::Square { side_length } => {
                calc_posotion_in_square_distance(
                    get_from_stdin("  Camera index")?,
                    get_camera_distance_in_square(
                        *side_length,
                        fov
                    ),
                )
            },
            SetupType::Free => {
                Position::new(
                    get_from_stdin("  x: ")?,
                    get_from_stdin("  y: ")?,
                    get_from_stdin("  rotation: ")?,
                )
            },
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
    let sock = &UdpSocket::bind(("0.0.0.0", 0))
        .map_err(|_| "Couldn't create socket")?;

    let mut organizer = Organizer {
        buffer: [0; MAX_MESSAGE_LENGTH],
        setup_type,
        hosts,
        sock,
    };

    loop {
        organizer.scan(own_ip)?;
        organizer.handle_commands()?;
    }
}
struct Organizer<'a, const BUFFER_SIZE: usize> {
    buffer: [u8; BUFFER_SIZE],
    hosts: &'a mut Vec<Host>,
    setup_type: SetupType,
    sock: &'a UdpSocket,
}

impl<const BUFFER_SIZE: usize> Organizer<'_, BUFFER_SIZE> {

    fn handle_commands(&mut self) -> Result<(), String> {
        let get_from_stdin: usize = get_from_stdin("Enter command: start (0) / stop (1) client: ")?;
        println!();
        match get_from_stdin {
            0 => self.start_client()?,
            1 =>  self.stop_client()?,
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

        let options = utils::print_hosts(
            self.hosts,
            |s| matches!(s,
                HostStatus::Client { status: ClientStatus::Idle, .. } |
                HostStatus::ConfiglessClient(ClientStatus::Idle)
            )
        );
        if options.is_empty() {
            println!("No clients found");
            return Ok(());
        }

        let selected: usize = get_from_stdin("\nSelect client to start: ")?;
        let host_index = *options.get(selected)
            .ok_or("No such index")?;
        let addr = ((self.hosts[host_index]).ip, MAIN_PORT);

        self.sock.send_to(&[Command::Start.into()], addr)
            .map_err(|_| "Couldn't send client start")?;

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
                vec![]
            ))
        } else {
            println!("Starting image stream");
            None
        };

        loop {
            self.sock.send_to(&[Command::RequestImage.into()], addr)
                .map_err(|_| "Couldn't request image")?;

            let timeout = self.sock.read_timeout().map_err(|_| "Couldn't get timeout???")?;
            self.sock.set_read_timeout(None).map_err(|_| "Couldn't set timeout???")?;

            let img = self.get_image((self.hosts[host_index]).ip)?;

            self.sock.set_read_timeout(timeout).map_err(|_| "Couldn't set timeout???")?;

            if let Some((board, imgs)) = &mut uncalibrated {
                let detection = calibration::find_board(&img, board, false)
                    .map_err(|_| "Couldn't find board")?;

                if let Some(fb) = detection {
                    let mut drawn_boards = img.clone();
                    calibration::draw_board(&mut drawn_boards, &fb)
                        .map_err(|_| "Couldn't draw detected boards")?;
                    display_image(&drawn_boards, "recieved", false)
                        .map_err(|_| "Couldn't display image")?;

                    let keep = get_from_stdin::<String>("  Keep image? (y) ")?.to_lowercase() == "y";
                    if keep {
                        imgs.push(img);
                    }

                    if imgs.is_empty() {
                        println!("  You can't calibrate with no images");
                        continue;
                    }
                } else {
                    display_image(&img, "recieved", false)
                        .map_err(|_| "Couldn't display image")?;

                    print!("  Board not found\n  ");
                }
            } else {
                display_image(&img, "recieved", false)
                    .map_err(|_| "Couldn't display image")?;
            }

            let more = get_from_stdin::<String>("  Continue? (y) ")?.to_lowercase() == "y";
            if !more {
                let _ = opencv::highgui::destroy_window("recieved");
                break;
            }
        }
        self.sock.send_to(&[Command::ImagesDone.into()], addr)
            .map_err(|_| "Couldn't send images done")?;

        let ip_bytes = server_ip.as_bytes();
        let ip_len = ip_bytes.len() as u16;
        
        let buff = if let Some((board, imgs)) = &uncalibrated {
            let calib = calibration::calibrate(board, imgs).map_err(|_| "Couldn't calibrate")?;
            let pos = self.setup_type.select_camera_position(calib.horizontal_fov)?;

            vec![
                pos.x.to_be_bytes().as_slice(),
                pos.y.to_be_bytes().as_slice(),
                pos.rotation.to_be_bytes().as_slice(),
                ip_len.to_be_bytes().as_slice(),
                ip_bytes,
                calib.to_be_bytes().as_slice(),
            ].concat()
        } else {
            let pos = self.setup_type.select_camera_position(f64::NAN)?;

            vec![
                pos.x.to_be_bytes().as_slice(),
                pos.y.to_be_bytes().as_slice(),
                pos.rotation.to_be_bytes().as_slice(),
                ip_len.to_be_bytes().as_slice(),
                ip_bytes,
            ].concat()
        };

        self.sock.send_to(&buff, addr)
            .map_err(|_| "Couldn't send position info and server address")?;

        Ok(())
    }

    fn stop_client(&mut self) -> Result<(), String> {
        let options = utils::print_hosts(
            self.hosts,
            |s| matches!(s,
                HostStatus::Client { status: ClientStatus::Running, .. } |
                HostStatus::ConfiglessClient(ClientStatus::Running)
            )
        );

        let selected: usize = get_from_stdin("\nSelect client to start: ")?;
        let host = options[selected];

        let addr = (self.hosts[host].ip, MAIN_PORT);
        self.sock.send_to(&[Command::Stop.into()], addr)
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

        self.sock.set_read_timeout(Some(TIMEOUT_DURATION))
            .map_err(|_| "Couldn't set timeout")?;

        match own_ip.broadcast() {
            Some(broadcast) if set_broadcast && !ip.is_loopback() =>
                self.scan_with_broadcast(broadcast),
            _ => {
                let netmask = own_ip.netmask()
                    .expect("No netmask");
                let netmask = if let IpAddr::V4(n) = netmask {
                    n
                } else {
                    unreachable!()
                };
                self.scan_with_template(
                    IPV4AddressTemplate::from_netmask(
                        ip,
                        scanning::get_netmask_bits(netmask) as usize,
                        scanning::TemplateMember::Fixed(MAIN_PORT)
                    )
                )
            }
        }
    }

    #[allow(unused, clippy::ptr_arg)]
    fn scan_with_template(&mut self, template: IPV4AddressTemplate) -> Result<(), &'static str> {
        todo!()
    }

    fn scan_with_broadcast(&mut self, broadcast: IpAddr) -> Result<(), &'static str> {
        self.sock.send_to(&[Command::Ping.into()], (broadcast, MAIN_PORT))
            .map_err(|_| "Couldn't send ping")?;

        let till = Instant::now() + WAIT_DURATION;

        let mut hit_hosts = vec![false; self.hosts.len()];

        'loopy: while Instant::now() < till {
            let Ok((msg_len, addr)) = self.sock.recv_from(&mut self.buffer) else {
                continue;
            };
            if msg_len != 1 {
                continue;
            }

            let ip = addr.ip();
            let Ok(status) = TryInto::<HostStatus>::try_into(self.buffer[0]) else {
                continue 'loopy;
            };

            let h: _ = self.hosts.iter_mut()
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
                HostStatus::ConfiglessClient(_) => HostStatus::ConfiglessClient(ClientStatus::Unreachable),
                HostStatus::Client { calibrated, .. } => HostStatus::Client {
                    status: ClientStatus::Unreachable,
                    calibrated,
                },
                HostStatus::Server(_) => HostStatus::Server(ServerStatus::Unreachable),
            };
        }

        Ok(())
    }

    fn recieve_from_host(&mut self, ip: IpAddr) -> Result<usize, &'static str> {
        loop {
            let (len, addr) = self.sock.recv_from(&mut self.buffer)
                .map_err(|_| "Couldn't recive data")?;
            if addr.ip() == ip {
                return Ok(len);
            }
        }
    }

    fn get_image(&mut self, ip: IpAddr) -> Result<Mat, &'static str> {
        let len = self.recieve_from_host(ip)?;

        if len < size_of::<u64>() {
            return Err("No length provided");
        }

        let mut img_size = u64::from_be_bytes(
            self.buffer[..8].try_into()
                .map_err(|_| "Not eight bytes???")?
        ) as usize + size_of::<u64>() - len;

        let mut img_buffer = core::Vector::from_slice(&self.buffer[8..]);
        img_buffer.reserve(img_size);

        while img_size != 0 {
            let len = self.recieve_from_host(ip)?;
            if len < size_of::<u64>() {
                continue;
            }

            img_buffer.extend(self.buffer[..len].to_vec());
            img_size -= len;
        }

        imgcodecs::imdecode(
            &img_buffer,
            imgcodecs::IMREAD_COLOR
        ).map_err(|_| "Couldn't decode image")
    }
}

const TIMEOUT_DURATION: Duration = Duration::from_millis(500);
const WAIT_DURATION:    Duration = Duration::from_millis(TIMEOUT_DURATION.as_millis() as u64 * 4);


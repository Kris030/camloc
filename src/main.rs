mod scanning;
mod utils;

use camloc_common::{get_from_stdin, hosts::{constants::{MAIN_PORT, MAX_MESSAGE_LENGTH}, HostStatus, ClientStatus, ServerStatus, Command}, position::{Position, get_camera_distance_in_square, calc_posotion_in_square_distance}, calibration};
use network_interface::{NetworkInterfaceConfig, NetworkInterface, Addr};
use std::{net::{IpAddr, UdpSocket}, time::{Duration, Instant}};
use opencv::{prelude::*, imgcodecs};

pub(crate) struct Host {
    pub(crate) status: HostStatus,
    pub(crate) ip: IpAddr,
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

#[derive(Clone, Copy)]
enum SetupType {
    Square { side_length: f64, fov: Option<f64> },
    Free,
}

impl SetupType {
    fn select_camera_position(&self) -> Result<Position, &'static str> {
        println!("Enter camera position");
        Ok(match self {
            SetupType::Square { side_length, fov } => {
                calc_posotion_in_square_distance(
                    get_from_stdin("  Camera index")?,
                    get_camera_distance_in_square(
                        *side_length,
                        fov.ok_or("Unknown fov for square setup")?
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
            fov: None,
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
            let width: u16 = get_from_stdin("  Charuco board width: ")?;
            let height: u16 = get_from_stdin("  Charuco board height: ")?;

            Some((
                calibration::generate_board(width as i32, height as i32)
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

            let img = self.get_image((self.hosts[host_index]).ip)?;
            if let Some((board, imgs)) = &mut uncalibrated {
                let detection = calibration::find_board(board, &img)
                    .map_err(|_| "Couldn't find board")?;

                let title = &format!("Image {}", imgs.len());
                if let Some((cs, ids)) = detection {
                    let drawn_boards = calibration::draw_boards(&img, &cs, &ids)
                        .map_err(|_| "Couldn't draw detected boards")?;
                    calibration::display_image(&drawn_boards, title)
                        .map_err(|_| "Couldn't display image")?;


                    let keep = get_from_stdin::<String>("  Keep image? (y)")?.to_lowercase() == "y";
                    if keep {
                        imgs.push(img);
                    }
                } else {
                    calibration::display_image(&img, title)
                        .map_err(|_| "Couldn't display image")?;
                }
            }

            let more = get_from_stdin::<String>("  Continue?")?.to_lowercase() != "y";
            if more {
                break;
            }
        }
        self.sock.send_to(&[Command::ImagesDone.into()], addr)
            .map_err(|_| "Couldn't send images done")?;

        let ip_bytes = server_ip.as_bytes();
        let ip_len = ip_bytes.len() as u16;
        
        let mut buff = vec![];

        let (pos, fov) = if let Some((board, imgs)) = &uncalibrated {
            let calib = calibration::calibrate(board, imgs).map_err(|_| "Couldn't calibrate")?;
            let fov = calib.fov.horizontal;
            if let SetupType::Square { fov: sfov, .. } = &mut self.setup_type {
                *sfov = Some(fov);
            }
            let pos = self.setup_type.select_camera_position()?;

            (pos, fov)
        } else {
            let pos = self.setup_type.select_camera_position()?;
            (pos, f64::NAN)
        };

        buff.copy_from_slice(&pos.x.to_be_bytes());
        buff.copy_from_slice(&pos.y.to_be_bytes());
        buff.copy_from_slice(&pos.rotation.to_be_bytes());

        if uncalibrated.is_some() {
            buff.copy_from_slice(&fov.to_be_bytes());
        }

        buff.copy_from_slice(&ip_len.to_be_bytes());
        buff.copy_from_slice(ip_bytes);

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
            Some(broadcast) if set_broadcast => self.scan_with_broadcast(broadcast),
            _ => self.scan_with_netmask(ip, own_ip.netmask().expect("No netmask"))
        }
    }

    #[allow(unused, clippy::ptr_arg)]
    fn scan_with_netmask(&mut self, ip: std::net::Ipv4Addr, netmask: IpAddr) -> Result<(), &'static str> {
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
            let (len, addr) = self.sock.peek_from(&mut self.buffer)
                .map_err(|_| "Couldn't recive data")?;
            if addr.ip() == ip {
                return Ok(len);
            }
        }
    }

    fn get_image(&mut self, ip: IpAddr) -> Result<Mat, &'static str> {
        let len = self.recieve_from_host(ip)?;

        if len < std::mem::size_of::<u64>() {
            return Err("No length provided");
        }

        let mut img_size = u64::from_be_bytes(self.buffer[..=8].try_into().map_err(|_| "Not eight bytes???")?) as usize;
        let mut img_buffer = Mat::from_slice(&self.buffer[8..])
            .map_err(|_| "Coulnd't create image buffer")?;

        if img_size <= BUFFER_SIZE - std::mem::size_of::<u64>() {
            return imgcodecs::imdecode(
                &img_buffer,
                imgcodecs::IMREAD_COLOR
            ).map_err(|_| "Couldn't decode image")
        }
        img_size -= BUFFER_SIZE - std::mem::size_of::<u64>();

        while img_size != 0 {
            let len = self.recieve_from_host(ip)?;

            if len < std::mem::size_of::<u64>() {
                continue;
            }

            let b = Mat::from_slice(&self.buffer[..len])
                .map_err(|_| "Couldn't create mat buffer")?;

            img_buffer.push_back(&b)
                .map_err(|_| "Couldn't push back mat buffer")?;

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


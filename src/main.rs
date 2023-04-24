mod scanning;
mod utils;
mod info;

use network_interface::{NetworkInterfaceConfig, NetworkInterface, Addr};
use std::{net::{IpAddr, UdpSocket}, time::{Duration, Instant}};
use info::{ServerStatus, ClientStatus, Host, HostStatus};

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

    let ai: usize = utils::get_from_stdin("\nEnter ip index: ")?;

    rnis.get(ai)
        .copied()
        .ok_or("Invalid index".to_string())
}

fn main() -> Result<(), String> {
    let own_ip = get_own_ip()?;
    println!("Selected {}\n", own_ip.ip());

    let mut hosts = vec![];
    let sock = UdpSocket::bind(("0.0.0.0", 0))
        .map_err(|_| "Couldn't create socket")?;

    loop {
        scan(&sock, own_ip, &mut hosts)?;
        handle_commands(&sock, &mut hosts)?;
    }
}

fn handle_commands(sock: &UdpSocket, hosts: &mut Vec<Host>) -> Result<(), String> {
    let get_from_stdin: usize = utils::get_from_stdin("Enter command: start (0) / stop (1) client: ")?;
    println!();
    match get_from_stdin {
        0 => start_client(sock, hosts)?,
        1 =>  stop_client(sock, hosts)?,
        _ => (),
    }
    println!();
    Ok(())
}

fn start_client(sock: &UdpSocket, hosts: &mut[Host]) -> Result<(), String> {
    let server = match utils::get_server(hosts) {
        Ok(s) => s,
        Err(count) => {
            println!("{count} servers running, resolve first");
            return Ok(());
        }
    };
    let server_ip = server.ip.to_string();

    let options = utils::print_hosts(hosts, |s| matches!(s, HostStatus::Client(ClientStatus::Idle)));
    if options.is_empty() {
        println!("No clients found");
        return Ok(());
    }

    let selected: usize = utils::get_from_stdin("\nSelect client to start: ")?;
    let host_index = *options.get(selected)
        .ok_or("No such index")?;
    let host = &mut hosts[host_index];

    let addr = (host.ip, TARGET_PORT);
    sock.send_to(START_CLIENT, addr)
        .map_err(|_| "Couldn't send client start")?;

    // TODO: recieve image + calibrate
    let (x, y, rotation, fov) = (0f64, 0f64, 0f64, 0f64);

    let ip_bytes = server_ip.as_bytes();
    let ip_len = ip_bytes.len() as u16;

    let buff = [
        x.to_be_bytes().as_slice(),
        y.to_be_bytes().as_slice(),
        rotation.to_be_bytes().as_slice(),
        fov.to_be_bytes().as_slice(),
        ip_len.to_be_bytes().as_slice(),
        ip_bytes,
    ].concat();

    sock.send_to(&buff, addr)
        .map_err(|_| "Couldn't send position info and server address")?;

    Ok(())
}

fn stop_client(sock: &UdpSocket, hosts: &mut Vec<Host>) -> Result<(), String> {
    let options = utils::print_hosts(hosts, |s| matches!(s, HostStatus::Client(ClientStatus::Running)));

    let selected: usize = utils::get_from_stdin("\nSelect client to start: ")?;
    let host = options[selected];

    let addr = (hosts[host].ip, TARGET_PORT);
    sock.send_to(STOP_CLIENT, addr)
        .map_err(|_| "Couldn't send client start")?;

    hosts.remove(host);

    Ok(())
}

fn scan(sock: &UdpSocket, own_ip: Addr, hosts: &mut Vec<Host>) -> Result<(), String> {
    println!("Scanning...\n");
    let IpAddr::V4(ip) = own_ip.ip() else {
        unreachable!()
    };
    
    let set_broadcast = sock.set_broadcast(true).is_ok();

    sock.set_read_timeout(Some(TIMEOUT_DURATION))
        .map_err(|_| "Couldn't set timeout")?;

    match own_ip.broadcast() {
        Some(broadcast) if set_broadcast => scan_with_broadcast(sock, hosts, broadcast),
        _ => scan_with_netmask(sock, hosts, ip, own_ip.netmask().expect("No netmask"))
    }
}

#[allow(unused, clippy::ptr_arg)]
fn scan_with_netmask(sock: &UdpSocket, info: &mut Vec<Host>, ip: std::net::Ipv4Addr, netmask: IpAddr) -> Result<(), String> {
    todo!()
}

fn scan_with_broadcast(sock: &UdpSocket, hosts: &mut Vec<Host>, broadcast: IpAddr) -> Result<(), String> {
    sock.send_to(PING, (broadcast, TARGET_PORT))
        .map_err(|_| "Couldn't send ping")?;

    let till = Instant::now() + WAIT_DURATION;
    let mut buff = [0; 64];

    let mut hit_hosts = vec![false; hosts.len()];

    'loopy: while Instant::now() < till {
        let Ok((msg_len, addr)) = sock.recv_from(&mut buff) else {
            continue;
        };
        if msg_len != 1 {
            continue;
        }

        let ip = addr.ip();
        let status = match buff[0] {
            SERVER_ANSWER => HostStatus::Server(ServerStatus::Running),
            CLIENT_ACTIVE => HostStatus::Client(ClientStatus::Running),
            CLIENT_IDLE   => HostStatus::Client(ClientStatus::Idle),

            _ => continue 'loopy,
        };

        let new = 'blocky: {
            for (h, hit) in hosts.iter_mut().zip(hit_hosts.iter_mut()) {
                if h.ip != ip {
                    continue;
                }

                *hit = true;
                h.status = status.clone();
                break 'blocky false;
            }
            true
        };
        if new {
            hosts.push(Host { status, ip });
        }
    }

    for (h, hit) in hosts.iter_mut().zip(hit_hosts.iter()) {
        if *hit {
            continue;
        }
        h.status = match h.status {
            HostStatus::Client(_) => HostStatus::Client(ClientStatus::Unreachable),
            HostStatus::Server(_) => HostStatus::Server(ServerStatus::Unreachable),
        };
    }

    Ok(())
}

const TIMEOUT_DURATION: Duration = Duration::from_millis(500);
const WAIT_DURATION: Duration = Duration::from_millis(TIMEOUT_DURATION.as_millis() as u64 * 4);

const TARGET_PORT: u16 = 1111;

const PING:         &[u8] = &[0x0b];
const START_CLIENT: &[u8] = &[0x60];
const STOP_CLIENT:  &[u8] = &[0xcd];

const SERVER_ANSWER: u8 = 0x5a;
const CLIENT_ACTIVE: u8 = 0xca;
const CLIENT_IDLE:   u8 = 0xcf;

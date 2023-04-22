mod scanning;

use network_interface::{NetworkInterfaceConfig, NetworkInterface, Addr};
use scanning::IPV4AddressTemplate;
use std::{io::{stdin, Write}, net::{IpAddr, Ipv6Addr, UdpSocket}};

fn get_own_ip() -> Result<Addr, String> {
    let nis = NetworkInterface::show()
        .map_err(|_| "Couldn't get network interfaces")?;
    let mut rnis = vec![];

    let mut ai = 0;
    for n in &nis {
        println!("Interface {}", n.name);
        for a in &n.addr {
            let ip = a.ip();
            if !ip.is_ipv4() {
                continue;
            }

            rnis.push(*a);
            ai += 1;
            println!("{ai:<3}{ip}");
        }
    }

    print!("\nEnter ip index: ");
    std::io::stdout().flush().unwrap();

    let mut l = String::new();
    stdin().read_line(&mut l)
        .map_err(|_| "Couldn't get line")?;

    let ai: usize = l[..(l.len() - 1)].parse()
        .map_err(|_| "Invalid index")?;

    rnis.get(ai - 1)
        .copied()
        .ok_or("Invalid index".to_string())
}

fn main() -> Result<(), String> {
    const port: u16 = 1111;

    let own_ip = get_own_ip()?;
    println!("{}", own_ip.ip());

    let mut clients = vec![];
    let sock = UdpSocket::bind(("0.0.0.0", port))
        .map_err(|_| "Couldn't create socket")?;

    loop {
        scan(&sock, own_ip, &mut clients)?;
        handle_commands(&sock, &mut clients)?;
    }
}

fn handle_commands(sock: &UdpSocket, clients: &mut Vec<Client>) -> Result<(), String> {
    todo!()
}

struct Client {

}

fn scan(sock: &UdpSocket, own_ip: Addr, clients: &mut Vec<Client>) -> Result<(), String> {
    let IpAddr::V4(ip) = own_ip.ip() else {
        unreachable!()
    };
    
    if let Some(broadcast) = own_ip.broadcast() {
        scan_with_broadcast(sock, clients, broadcast)
    } else {
        let netmask = own_ip.netmask()
            .expect("No netmask");
        scan_with_netmask(sock, clients, ip, netmask)
    }
}

fn scan_with_netmask(sock: &UdpSocket, clients: &[Client], ip: std::net::Ipv4Addr, netmask: IpAddr) -> Result<(), String> {
    todo!()
}

fn scan_with_broadcast(sock: &UdpSocket, clients: &[Client], broadcast: IpAddr) -> Result<(), String> {
    todo!()
}


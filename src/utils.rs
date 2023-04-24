use std::{io::{stdin, Write}, str::FromStr};

use crate::info::{HostStatus, Host, ServerStatus};

pub(crate) fn get_from_stdin<T: FromStr>(prompt: &str) -> Result<T, &'static str> {
    
    print!("{prompt}");
    std::io::stdout().flush().unwrap();

    let mut l = String::new();
    stdin().read_line(&mut l)
        .map_err(|_| "Couldn't get line")?;

    l[..(l.len() - 1)].parse()
        .map_err(|_| "Invalid index")
}

pub(crate) fn print_hosts<F: FnMut(&HostStatus) -> bool>(hosts: &mut [Host], mut filter: F) -> Vec<usize> {
    println!("Available hosts");
    let mut ret = vec![];

    let mut i = 0;
    for (j, h) in hosts.iter().enumerate() {
        let ip = h.ip;

        let fres = filter(&h.status);
        if fres {
            print!("{i:<3}");
            i += 1;
        } else {
            print!("   ");
        }

        match &h.status {
            HostStatus::Client(_) => println!("CLIENT {ip}"),
            HostStatus::Server(_) => println!("SERVER {ip}"),
        }

        if fres {
            ret.push(j);
        }
    }

    ret
}

pub(crate) fn get_server(hosts: &mut[Host]) -> Result<&mut Host, usize> {
    let mut si = Err(0);

    for (i, h) in hosts.iter().enumerate() {
        if let HostStatus::Server(ServerStatus::Running) = h.status {
            si = match si {
                Ok(_) => Err(1),
                Err(0) => Ok(i),
                Err(n) => Err(n + 1),
            };
        }
    }

    si.map(|si| &mut hosts[si])
}

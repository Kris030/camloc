use camloc_common::hosts::{HostStatus, ServerStatus};
use crate::Host;

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
            HostStatus::Client { calibrated, .. } => {
                print!("CLIENT {ip}{}", if *calibrated {
                    " CALIBRATED"
                } else {
                    ""
                });
            },
            HostStatus::ConfiglessClient(_) => println!("PHONE  {ip}"),
            HostStatus::Server(_)           => println!("SERVER {ip}"),
        }

        if fres {
            ret.push(j);
        }
    }

    println!();

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

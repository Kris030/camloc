use crate::Host;
use camloc_common::hosts::{HostInfo, HostState, HostType};

pub(crate) fn print_hosts<F: FnMut(&HostInfo) -> bool>(
    hosts: &mut [Host],
    mut filter: F,
) -> Vec<usize> {
    println!("Available hosts");
    let mut ret = vec![];

    let mut i = 0;
    for (j, h) in hosts.iter().enumerate() {
        let ip = h.ip;

        let fres = filter(&h.info);
        if fres {
            print!("{i:<3}");
            i += 1;
        } else {
            print!("   ");
        }

        match &h.info.host_type {
            HostType::Client { calibrated, .. } => {
                print!("CLIENT {ip}");
                if *calibrated {
                    print!(" CALIBRATED");
                }
                println!();
            }
            HostType::ConfiglessClient => println!("PHONE  {ip}"),
            HostType::Server => println!("SERVER {ip}"),
        }

        if fres {
            ret.push(j);
        }
    }

    println!();

    ret
}

pub(crate) fn get_server(hosts: &mut [Host]) -> Result<&mut Host, usize> {
    let mut si = Err(0);

    for (i, h) in hosts.iter().enumerate() {
        if let HostInfo {
            host_type: HostType::Server,
            host_state: HostState::Running | HostState::Idle,
        } = h.info
        {
            si = match si {
                Ok(_) => Err(1),
                Err(0) => Ok(i),
                Err(n) => Err(n + 1),
            };
        }
    }

    si.map(|si| &mut hosts[si])
}

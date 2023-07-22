use camloc_common::hosts::{HostInfo, HostState, HostType};

use crate::Host;

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

use std::net::IpAddr;

pub(crate) struct Host {
    pub(crate) status: HostStatus,
    pub(crate) ip: IpAddr,
}

#[derive(Clone)]
pub(crate) enum HostStatus {
    Client(ClientStatus),
    Server(ServerStatus),
}

#[derive(Clone)]
pub(crate) enum ClientStatus {
    Unreachable,
    Running,
    Idle,
}

#[derive(Clone)]
pub(crate) enum ServerStatus {
    Unreachable,
    Running,
}

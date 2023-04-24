#![allow(dead_code)]

use std::net::IpAddr;

pub(crate) struct Coordinate {
    x: f64,
    y: f64,
}
pub(crate) struct Position {
    pub(crate) coords: Coordinate,
    pub(crate) rotation: f64
}

pub(crate) struct Host {
    pub(crate) status: HostStatus,
    pub(crate) ip: IpAddr,
}

pub(crate) enum HostStatus {
    Client(ClientStatus),
    Server(ServerStatus),
}

pub(crate) enum ClientStatus {
    Unreachable,
    Running,
    Idle,
}

pub(crate) enum ServerStatus {
    Unreachable,
    Running,
}

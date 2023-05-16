use std::mem::size_of;

use crate::position::Position;

#[allow(clippy::unusual_byte_groupings)]
pub mod constants {
    pub const UDP_MAX_MESSAGE_LENGTH: usize = 65507;

    pub const MAIN_PORT: u16 = 0xdddd;
    pub const ORGANIZER_STARTER_PORT: u16 = 0xdddb;

    pub mod status_reply {
        pub mod host_type {
            pub const CONFIGLESS: u8 = 0b10_0_0_0000;
            pub const CLIENT: u8 = 0b01_0_0_0000;
            pub const SERVER: u8 = 0b11_0_0_0000;
        }

        pub mod state {
            pub const RUNNING: u8 = 0b00_1_0_0000;
            pub const IDLE: u8 = 0b00_0_0_0000;
        }

        pub mod masks {
            pub const HOST_TYPE: u8 = 0b11_0_0_0000;
            pub const STATE: u8 = 0b00_1_0_0000;
            pub const CALIBRATED: u8 = 0b00_0_1_0000;

            pub const ONES: u8 = 0xff;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HostType {
    ConfiglessClient,
    Server,
    Client { calibrated: bool },
}

#[derive(Debug, Clone, Copy)]
pub struct HostInfo {
    pub host_type: HostType,
    pub host_state: HostState,
}

impl TryInto<u8> for HostInfo {
    type Error = ();

    fn try_into(self) -> Result<u8, Self::Error> {
        use constants::status_reply::{host_type::*, masks, state::*};
        use HostState::*;
        use HostType::*;

        Ok(match self.host_type {
            Client { calibrated } => {
                CLIENT
                    | (if calibrated {
                        masks::CALIBRATED & masks::ONES
                    } else {
                        0
                    })
            }

            ConfiglessClient => CONFIGLESS,
            Server => SERVER,
        } | (match self.host_state {
            Unreachable => return Err(()),
            Running => RUNNING,
            Idle => IDLE,
        }))
    }
}

impl TryFrom<u8> for HostInfo {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        use constants::status_reply::{host_type::*, masks, state::*};
        use HostState::*;
        use HostType::*;

        let host_type = match v & masks::HOST_TYPE {
            CONFIGLESS => ConfiglessClient,

            CLIENT => {
                let calibrated = (v & masks::CALIBRATED) != 0;
                Client { calibrated }
            }

            SERVER => Server,

            _ => return Err(()),
        };

        let host_state = match v & masks::STATE {
            RUNNING => Running,
            IDLE => Idle,
            _ => return Err(()),
        };

        Ok(HostInfo {
            host_type,
            host_state,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HostState {
    Unreachable,
    Running,
    Idle,
}

#[derive(Clone, Copy)]
#[must_use]
pub enum Command<'a> {
    Ping,

    Connect { position: Position, fov: f64 },
    Disconnect,

    Start,
    StartServer { cube: [u8; 4] },
    StartConfigless { ip: &'a str },
    Stop,

    RequestImage,
    ImagesDone,

    ValueUpdate(ClientData),
    InfoUpdate { position: Position, fov: f64 },
}
impl Command<'_> {
    pub const PING: u8 = 0x0b;
    pub const CONNECT: u8 = 0xcc;
    pub const DISCONNECT: u8 = 0xdc;
    pub const START: u8 = 0x60;
    pub const START_SERVER: u8 = 0x55;
    pub const START_CONFIGLESS: u8 = 0x6c;
    pub const STOP: u8 = 0xcd;
    pub const REQUEST_IMAGE: u8 = 0x17;
    pub const IMAGES_DONE: u8 = 0x1d;
    pub const VALUE_UPDATE: u8 = 0x21;
    pub const INFO_UPDATE: u8 = 0x1f;
}

impl From<Command<'_>> for Vec<u8> {
    fn from(value: Command) -> Self {
        match value {
            Command::Ping => vec![Command::PING],

            Command::Connect { position, fov } => [
                Command::CONNECT.to_be_bytes().as_slice(),
                position.to_be_bytes().as_slice(),
                fov.to_be_bytes().as_slice(),
            ]
            .concat(),

            Command::Disconnect => vec![Command::DISCONNECT],

            Command::Start => vec![Command::START],
            Command::StartServer { cube } => vec![
                Command::START_SERVER.to_be_bytes().as_slice(),
                cube.map(u8::to_be).as_slice(),
            ]
            .concat(),

            Command::StartConfigless { ip } => [
                Command::START_CONFIGLESS.to_be_bytes().as_slice(),
                (ip.len() as u16).to_be_bytes().as_slice(),
                ip.as_bytes(),
            ]
            .concat(),

            Command::Stop => vec![Command::STOP],

            Command::RequestImage => vec![Command::REQUEST_IMAGE],

            Command::ImagesDone => vec![Command::IMAGES_DONE],

            Command::ValueUpdate(ClientData {
                marker_id,
                target_x_position: value,
                rotation,
            }) => [
                Command::VALUE_UPDATE.to_be_bytes().as_slice(),
                marker_id.to_be_bytes().as_slice(),
                value.to_be_bytes().as_slice(),
                rotation.0.to_be_bytes().as_slice(),
                rotation.1.to_be_bytes().as_slice(),
                rotation.2.to_be_bytes().as_slice(),
            ]
            .concat(),

            Command::InfoUpdate { position, fov } => [
                Command::INFO_UPDATE.to_be_bytes().as_slice(),
                position.to_be_bytes().as_slice(),
                fov.to_be_bytes().as_slice(),
            ]
            .concat(),
        }
    }
}

impl<'a> TryFrom<&'a [u8]> for Command<'a> {
    type Error = ();

    fn try_from(buf: &'a [u8]) -> Result<Self, Self::Error> {
        let len = buf.len();
        if len < 1 {
            return Err(());
        }

        // without the command byte
        let cmd = buf[0];
        let len = len - 1;
        let buf = &buf[1..];

        Ok(match cmd {
            Command::PING if len == 0 => Command::Ping,
            Command::START if len == 0 => Command::Start,
            Command::STOP if len == 0 => Command::Stop,
            Command::REQUEST_IMAGE if len == 0 => Command::RequestImage,
            Command::IMAGES_DONE if len == 0 => Command::ImagesDone,
            Command::DISCONNECT if len == 0 => Command::Disconnect,

            Command::VALUE_UPDATE if len == 1 + 4 * size_of::<f64>() => {
                Command::ValueUpdate(ClientData {
                    marker_id: u8::from_be(buf[0]),
                    target_x_position: f64::from_be_bytes(
                        buf[1..size_of::<f64>() + 1].try_into().map_err(|_| ())?,
                    ),
                    rotation: (
                        f64::from_be_bytes(
                            buf[size_of::<f64>() + 1..2 * size_of::<f64>() + 1]
                                .try_into()
                                .map_err(|_| ())?,
                        ),
                        f64::from_be_bytes(
                            buf[2 * size_of::<f64>() + 1..3 * size_of::<f64>() + 1]
                                .try_into()
                                .map_err(|_| ())?,
                        ),
                        f64::from_be_bytes(
                            buf[3 * size_of::<f64>() + 1..4 * size_of::<f64>() + 1]
                                .try_into()
                                .map_err(|_| ())?,
                        ),
                    ),
                })
            }

            Command::CONNECT if len == 4 * size_of::<f64>() => Command::Connect {
                position: Position::from_be_bytes(&buf[..24].try_into().unwrap()),
                fov: f64::from_be_bytes(
                    buf[3 * size_of::<f64>()..4 * size_of::<f64>()]
                        .try_into()
                        .map_err(|_| ())?,
                ),
            },

            Command::INFO_UPDATE if len == 4 * size_of::<f64>() => Command::InfoUpdate {
                position: Position::from_be_bytes(&buf[..24].try_into().unwrap()),
                fov: f64::from_be_bytes(
                    buf[3 * size_of::<f64>()..4 * size_of::<f64>()]
                        .try_into()
                        .map_err(|_| ())?,
                ),
            },

            Command::START_CONFIGLESS if len >= size_of::<u16>() => {
                let ip_len = u16::from_be_bytes(buf[..size_of::<u16>()].try_into().map_err(|_| ())?)
                    as usize;
                if len != size_of::<u16>() + ip_len {
                    return Err(());
                }

                Command::StartConfigless {
                    ip: std::str::from_utf8(&buf[size_of::<u16>()..size_of::<u16>() + ip_len])
                        .map_err(|_| ())?,
                }
            }

            Command::START_SERVER if len == 4 => Command::StartServer {
                cube: [buf[0], buf[1], buf[2], buf[3]],
            },

            _ => return Err(()),
        })
    }
}

impl<'a> TryFrom<&'a mut [u8]> for Command<'a> {
    type Error = ();
    fn try_from(buf: &'a mut [u8]) -> Result<Self, Self::Error> {
        (&*buf).try_into()
    }
}

#[derive(Clone, Copy)]
pub struct ClientData {
    pub marker_id: u8,
    pub target_x_position: f64,
    pub rotation: (f64, f64, f64),
}
impl ClientData {
    pub fn new(marker_id: u8, target_x_position: f64, rotation: (f64, f64, f64)) -> ClientData {
        Self {
            marker_id,
            target_x_position,
            rotation,
        }
    }
}

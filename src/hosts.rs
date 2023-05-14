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
pub enum HostStatus {
    ConfiglessClient(ClientStatus),
    Server(ServerStatus),
    Client {
        status: ClientStatus,
        calibrated: bool,
    },
}

impl TryInto<u8> for HostStatus {
    type Error = ();

    fn try_into(self) -> Result<u8, Self::Error> {
        use constants::status_reply::{host_type::*, masks, state::*};
        use ClientStatus::*;
        use HostStatus::*;

        Ok(match self {
            Client { status, calibrated } => {
                CLIENT
                    | (match status {
                        Unreachable => return Err(()),
                        Running => RUNNING,
                        Idle => IDLE,
                    })
                    | (if calibrated {
                        masks::CALIBRATED & masks::ONES
                    } else {
                        0
                    })
            }

            ConfiglessClient(s) => {
                CONFIGLESS
                    | (match s {
                        Unreachable => return Err(()),
                        Running => RUNNING,
                        Idle => IDLE,
                    })
            }

            Server(s) => {
                SERVER
                    | (match s {
                        ServerStatus::Unreachable => return Err(()),
                        ServerStatus::Running => RUNNING,
                    })
            }
        })
    }
}

impl TryFrom<u8> for HostStatus {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        use constants::status_reply::{host_type::*, masks, state::*};
        use ClientStatus::*;
        use HostStatus::*;

        match v & masks::HOST_TYPE {
            CONFIGLESS => match v & masks::STATE {
                RUNNING => Ok(ConfiglessClient(Running)),
                IDLE => Ok(ConfiglessClient(Idle)),
                _ => Err(()),
            },

            CLIENT => {
                let calibrated = (v & masks::CALIBRATED) != 0;
                match v & masks::STATE {
                    RUNNING => Ok(Client {
                        calibrated,
                        status: Running,
                    }),
                    IDLE => Ok(Client {
                        calibrated,
                        status: Idle,
                    }),
                    _ => Err(()),
                }
            }

            SERVER => Ok(Server(ServerStatus::Running)),

            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ClientStatus {
    Unreachable,
    Running,
    Idle,
}

#[derive(Debug, Clone, Copy)]
pub enum ServerStatus {
    Unreachable,
    Running,
}

#[derive(Clone, Copy)]
#[must_use]
pub enum Command<'a> {
    Ping,

    Connect { position: Position, fov: f64 },

    Start,
    StartConfigless { ip: &'a str },
    Stop,

    RequestImage,
    ImagesDone,

    ValueUpdate(f64),
    InfoUpdate { position: Position, fov: f64 },
}
impl Command<'_> {
    pub const PING: u8 = 0x0b;
    pub const CONNECT: u8 = 0xcc;
    pub const START: u8 = 0x60;
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
                position.x.to_be_bytes().as_slice(),
                position.y.to_be_bytes().as_slice(),
                position.rotation.to_be_bytes().as_slice(),
                fov.to_be_bytes().as_slice(),
            ]
            .concat(),

            Command::Start => vec![Command::START],

            Command::StartConfigless { ip } => [
                Command::START_CONFIGLESS.to_be_bytes().as_slice(),
                (ip.len() as u16).to_be_bytes().as_slice(),
                ip.as_bytes(),
            ]
            .concat(),

            Command::Stop => vec![Command::STOP],

            Command::RequestImage => vec![Command::REQUEST_IMAGE],

            Command::ImagesDone => vec![Command::IMAGES_DONE],

            Command::ValueUpdate(v) => [
                Command::VALUE_UPDATE.to_be_bytes().as_slice(),
                v.to_be_bytes().as_slice(),
            ]
            .concat(),

            Command::InfoUpdate { position, fov } => [
                Command::INFO_UPDATE.to_be_bytes().as_slice(),
                position.x.to_be_bytes().as_slice(),
                position.y.to_be_bytes().as_slice(),
                position.rotation.to_be_bytes().as_slice(),
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
        let len = len - 1;
        let cmd = buf[0];
        let buf = &buf[1..];

        Ok(match cmd {
            Command::PING if len == 0 => Command::Ping,
            Command::START if len == 0 => Command::Start,
            Command::STOP if len == 0 => Command::Stop,
            Command::REQUEST_IMAGE if len == 0 => Command::RequestImage,
            Command::IMAGES_DONE if len == 0 => Command::ImagesDone,

            Command::VALUE_UPDATE if len == size_of::<f64>() => Command::ValueUpdate(
                f64::from_be_bytes(buf[..size_of::<f64>()].try_into().map_err(|_| ())?),
            ),

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

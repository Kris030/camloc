use crate::Position;

#[allow(clippy::unusual_byte_groupings)]
pub mod constants {

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
    Client { calibrated: bool },
    ConfiglessClient,
    Server,
}

#[derive(Debug, Clone, Copy)]
pub struct HostInfo {
    pub host_state: HostState,
    pub host_type: HostType,
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

#[must_use]
#[derive(Clone, Copy)]
pub enum Command<'a> {
    Ping,

    Connect {
        position: Position,
        fov: f64,
    },
    ClientDisconnect,

    Start,
    StartServer {
        cube: [u8; 4],
    },
    StartConfigless {
        ip: &'a str,
    },
    Stop,

    RequestImage,
    ImagesDone,

    ValueUpdate(ClientData),
    InfoUpdate {
        client_ip: &'a str,
        position: Position,
        fov: Option<f64>,
    },
}

impl Command<'_> {
    pub const PING: u8 = 0x0b;
    pub const CONNECT: u8 = 0xcc;
    pub const CLIENT_DISCONNECT: u8 = 0xdc;
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

            Command::ClientDisconnect => vec![Command::CLIENT_DISCONNECT],

            Command::Start => vec![Command::START],
            Command::StartServer { cube } => [
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
                x_position: value,
            }) => [
                Command::VALUE_UPDATE.to_be_bytes().as_slice(),
                marker_id.to_be_bytes().as_slice(),
                value.to_be_bytes().as_slice(),
            ]
            .concat(),

            Command::InfoUpdate {
                client_ip,
                position,
                fov,
            } => {
                let ip = client_ip.as_bytes();
                let ip_len = (ip.len() as u16).to_be_bytes();
                let c = Command::INFO_UPDATE.to_be_bytes();
                let p = position.to_be_bytes();
                let f = if fov.is_some() { 1u8 } else { 0u8 }.to_be_bytes();

                let mut v = vec![
                    ip_len.as_slice(),
                    ip,
                    c.as_slice(),
                    p.as_slice(),
                    f.as_slice(),
                ];

                let fov = fov.map(f64::to_be_bytes);
                if let Some(fov) = &fov {
                    v.push(fov);
                }

                v.concat()
            }
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
        let buf = &buf[1..];

        (|| {
            Some(match cmd {
                Command::PING => Command::Ping,
                Command::START => Command::Start,
                Command::STOP => Command::Stop,
                Command::REQUEST_IMAGE => Command::RequestImage,
                Command::IMAGES_DONE => Command::ImagesDone,
                Command::CLIENT_DISCONNECT => Command::ClientDisconnect,

                Command::VALUE_UPDATE => Command::ValueUpdate(ClientData {
                    marker_id: u8::from_be(*buf.first()?),
                    x_position: f64::from_be_bytes(buf.get(1..9)?.try_into().ok()?),
                }),

                Command::CONNECT => Command::Connect {
                    position: Position::from_be_bytes(&buf.get(..24)?.try_into().ok()?),
                    fov: f64::from_be_bytes(buf.get(24..32)?.try_into().ok()?),
                },

                Command::INFO_UPDATE => {
                    let ip_len = u16::from_be_bytes(buf.get(..2)?.try_into().ok()?) as usize;
                    let client_ip = std::str::from_utf8(buf.get(2..2 + ip_len)?).unwrap();
                    let position = Position::from_be_bytes(&buf.get(..26)?.try_into().ok()?);
                    let fov = if *buf.get(26)? == 1 {
                        Some(f64::from_be_bytes(buf.get(27..35)?.try_into().ok()?))
                    } else {
                        None
                    };
                    Command::InfoUpdate {
                        client_ip,
                        position,
                        fov,
                    }
                }

                Command::START_CONFIGLESS => {
                    let ip_len = u16::from_be_bytes(buf.get(..2)?.try_into().ok()?) as usize;

                    Command::StartConfigless {
                        ip: std::str::from_utf8(buf.get(2..2 + ip_len)?).ok()?,
                    }
                }

                Command::START_SERVER => Command::StartServer {
                    cube: buf.get(..4)?.try_into().ok()?,
                },

                _ => return None,
            })
        })()
        .ok_or(())
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
    pub x_position: f64,
    pub marker_id: u8,
}

impl ClientData {
    pub fn new(marker_id: u8, x_position: f64) -> ClientData {
        Self {
            x_position,
            marker_id,
        }
    }
}

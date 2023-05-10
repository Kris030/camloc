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

#[derive(Clone, Copy)]
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
                let calibrated = v & masks::CALIBRATED != 0;
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

#[derive(Clone, Copy)]
pub enum ClientStatus {
    Unreachable,
    Running,
    Idle,
}

#[derive(Clone, Copy)]
pub enum ServerStatus {
    Unreachable,
    Running,
}

#[derive(Clone, Copy)]
pub enum Command {
    Ping = Command::PING as isize,

    Connect = Command::CONNECT as isize,

    Start = Command::START as isize,
    StartConfigless = Command::START_CONFIGLESS as isize,
    Stop = Command::STOP as isize,

    RequestImage = Command::REQUEST_IMAGE as isize,
    ImagesDone = Command::IMAGES_DONE as isize,

    ValueUpdate = Command::VALUE_UPDATE as isize,
    InfoUpdate = Command::INFO_UPDATE as isize,
}
impl Command {
    const PING: u8 = 0x0b;
    const CONNECT: u8 = 0xcc;
    const START: u8 = 0x60;
    const START_CONFIGLESS: u8 = 0x6c;
    const STOP: u8 = 0xcd;
    const REQUEST_IMAGE: u8 = 0x17;
    const IMAGES_DONE: u8 = 0x1d;
    const VALUE_UPDATE: u8 = 0x21;
    const INFO_UPDATE: u8 = 0x1f;
}

impl From<Command> for u8 {
    fn from(value: Command) -> Self {
        value as u8
    }
}

impl TryInto<Command> for u8 {
    type Error = ();

    fn try_into(self) -> Result<Command, Self::Error> {
        use Command::*;

        match self {
            x if x == Ping as u8 => Ok(Ping),
            x if x == Connect as u8 => Ok(Connect),

            x if x == Start as u8 => Ok(Start),
            x if x == StartConfigless as u8 => Ok(StartConfigless),
            x if x == Stop as u8 => Ok(Stop),

            x if x == RequestImage as u8 => Ok(RequestImage),
            x if x == ImagesDone as u8 => Ok(ImagesDone),

            x if x == ValueUpdate as u8 => Ok(ValueUpdate),
            x if x == InfoUpdate as u8 => Ok(InfoUpdate),

            _ => Err(()),
        }
    }
}

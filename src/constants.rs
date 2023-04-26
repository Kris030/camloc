
pub mod status_reply {
    pub mod host_type {
        pub const CONFIGLESS: u8 = 0b10_0_0_0000;
        pub const CLIENT:     u8 = 0b01_0_0_0000;
        pub const SERVER:     u8 = 0b11_0_0_0000;
    }

    pub mod state {
        pub const RUNNING:    u8 = 0b00_1_0_0000;
        pub const IDLE:       u8 = 0b00_0_0_0000;
    }

    pub mod masks {
        pub const HOST_TYPE:  u8 = 0b11_0_0_0000;
        pub const STATE:      u8 = 0b00_1_0_0000;
        pub const CALIBRATED: u8 = 0b00_0_1_0000;

    }
}

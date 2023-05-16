pub trait Compass {
    fn get_value(&mut self) -> std::io::Result<f64>;
}

#[cfg(feature = "serial-compass")]
pub struct SerialCompass<P: tokio_serial::SerialPort> {
    /// The offset of the compass compared to the server coordninate system
    compass_offset: f64,
    serial_port: P,
}

#[cfg(feature = "serial-compass")]
impl<P: tokio_serial::SerialPort> SerialCompass<P> {
    pub const START_SIGNAL: &[u8] = &[0x60];
    pub const STOP_SIGNAL: &[u8] = &[0xcc];

    pub fn start(
        mut serial_port: P,
        compass_offset: f64,
    ) -> Result<SerialCompass<P>, std::io::Error> {
        serial_port.write_all(Self::START_SIGNAL)?;
        Ok(Self {
            serial_port,
            compass_offset,
        })
    }
}

#[cfg(feature = "serial-compass")]
impl<P: tokio_serial::SerialPort> Compass for SerialCompass<P> {
    fn get_value(&mut self) -> std::io::Result<f64> {
        let mut angle = [0; std::mem::size_of::<f64>()];
        self.serial_port.read_exact(&mut angle)?;

        // microbit is little endian
        Ok(f64::from_le_bytes(angle) - self.compass_offset)
    }
}

#[cfg(feature = "serial-compass")]
impl<P: tokio_serial::SerialPort> Drop for SerialCompass<P> {
    fn drop(&mut self) {
        self.serial_port.write_all(Self::STOP_SIGNAL).unwrap();
    }
}

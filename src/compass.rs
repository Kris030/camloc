#[macro_export]
macro_rules! no_compass {
    () => {
        if true {
            None
        } else {
            async fn dummy() -> Option<f64> {
                None
            }
            Some(dummy)
        }
    };
}
pub use no_compass;

#[cfg(feature = "serial-compass")]
pub mod serial {
    use std::{sync::Arc, time::Duration};
    use tokio::sync::RwLock;
    use tokio_serial::SerialStream;

    pub struct SerialCompass {
        last_value: Arc<RwLock<Option<f64>>>,
    }

    impl SerialCompass {
        pub fn start(
            mut serial_port: SerialStream,
            compass_offset: f64,
        ) -> Result<SerialCompass, std::io::Error> {
            let last_value = Arc::new(RwLock::new(None));
            let lval2 = last_value.clone();

            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut i = tokio::time::interval(Duration::from_millis(10));
                loop {
                    i.tick().await;
                    serial_port.write_all(&[b'$']).await.unwrap();

                    let v = serial_port.read_u8().await;
                    let v = v.ok().map(|v| (v as f64).to_radians() - compass_offset);

                    let mut lv = lval2.write().await;
                    *lv = v;
                }
            });
            Ok(Self { last_value })
        }

        pub const DATA_SIGNAL: [u8; 1] = [b'$'];
        pub async fn get_value(&self) -> Option<f64> {
            *self.last_value.read().await
        }
    }
}

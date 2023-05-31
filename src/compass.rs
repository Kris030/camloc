#[macro_export]
macro_rules! no_compass {
    () => {
        if true {
            None
        } else {
            pub struct NoCompass;
            impl Compass for NoCompass {
                fn get_value(&mut self) -> Option<f64> {
                    None
                }
                fn stop(&mut self) {}
            }
            Some(Box::new(NoCompass))
        }
    };
}
pub use no_compass;

pub trait Compass {
    fn get_value(&mut self) -> Option<f64>;
    fn stop(&mut self);
}

#[cfg(feature = "serial-compass")]
pub mod serial {
    use std::{sync::Arc, time::Duration};
    use tokio::{sync::RwLock, task::JoinHandle};

    use super::Compass;

    pub struct SerialCompass {
        last_value: Arc<RwLock<Option<f64>>>,
        handle: Option<JoinHandle<()>>,
        run: Arc<RwLock<bool>>,
    }

    impl SerialCompass {
        pub fn start(
            mut serial_port: tokio_serial::SerialStream,
            compass_offset: f64,
        ) -> Result<SerialCompass, std::io::Error> {
            let last_value = Arc::new(RwLock::new(None));
            let lval2 = last_value.clone();

            let run = Arc::new(RwLock::const_new(true));
            let run2 = run.clone();

            let handle = Some(tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut i = tokio::time::interval(Duration::from_millis(10));
                loop {
                    let r = run2.read().await;
                    if !*r {
                        break;
                    }
                    drop(r);

                    i.tick().await;
                    serial_port.write_all(&[b'$']).await.unwrap();

                    let v = serial_port.read_u8().await;
                    let v = v.ok().map(|v| (v as f64).to_radians() - compass_offset);

                    let mut lv = lval2.write().await;
                    *lv = v;
                }
            }));

            Ok(Self {
                last_value,
                handle,
                run,
            })
        }

        pub const DATA_SIGNAL: [u8; 1] = [b'$'];
    }

    impl Compass for SerialCompass {
        fn get_value(&mut self) -> Option<f64> {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async { *self.last_value.read().await })
            })
        }

        fn stop(&mut self) {
            self.handle.take().expect("Should always be Some");
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let mut r = self.run.blocking_write();
                    *r = false;
                    drop(r);
                })
            });
        }
    }
}

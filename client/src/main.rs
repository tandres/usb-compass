use libusb::{Context, Device};
use std::time::Duration;
use common::usb::{VENDOR_ID, PROD_ID};

enum AppState {
    Connected,
    Scanning,
}

struct App {
    state: AppState,
    context: Context,
}

impl App {
    fn new() -> App {
        let context = libusb::Context::new().unwrap();

        App {
            state: AppState::Scanning,
            context,
        }
    }

    async fn turn(&mut self) {
        self.state = match self.state {
            AppState::Connected => {
                if self.connected_update() {
                    AppState::Connected
                } else {
                    AppState::Scanning
                }
            }
            AppState::Scanning => {
                tokio::time::sleep(Duration::from_millis(1000)).await;
                if let Some(device) = self.scan() {
                    let config_descriptor = device.active_config_descriptor()
                        .expect("Failed to get config descriptor");
                    println!("Device found! Config Descriptor: {:?}", config_descriptor);
                    let interfaces = config_descriptor.interfaces();
                    for interface in interfaces {
                        let number = interface.number();
                        println!("Interface Number: {}", number);
                        for descriptor in interface.descriptors() {
                            println!("{:#?}", descriptor);
                            for endpoint in descriptor.endpoint_descriptors() {
                                println!("Endpoint {}: \n\tXfer Type: {:?}\n\tUsage Type: {:?}\n\tDirection: {:?}\n\tAddr: {:?}",
                                    endpoint.number(),
                                    endpoint.transfer_type(),
                                    endpoint.usage_type(),
                                    endpoint.direction(),
                                    endpoint.address());
                            }
                        }
                    }
                    let mut handle = device.open().expect("Failed to open dev!");
                    let config = handle.active_configuration().expect("Failed to read config");
                    let langs = handle.read_languages(Duration::from_millis(500))
                        .expect("Failed to read langs");
                    println!("Langs: {:?}", langs);
                    // let config_str = handle.read_configuration_string(langs[0], &config_descriptor, Duration::from_millis(500)).expect("Failed to read config string");
                    // println!("Config: {} {}", config, config_str);
                    println!("Kernel driver: {:?}", handle.kernel_driver_active(config));
                    if handle.kernel_driver_active(config).expect("Failed to get kernel driver") {
                        handle
                            .detach_kernel_driver(config)
                            .expect("Failed to detach kernel driver");
                    }

                    handle.claim_interface(1).expect("Failed to claim interface");
                    let buf = b"test_string".clone();
                    handle.write_bulk(2, &buf, Duration::from_millis(500))
                        .expect("Failed to write bulk");
                    println!("{:?}", buf);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    let mut rbuf: [u8; 64] = [0; 64];
                    loop {
                        let res = handle.read_bulk(130, &mut rbuf, Duration::from_millis(500));
                        if let Ok(num) = res {
                            println!("Read {} bytes: {}", num, String::from_utf8_lossy(&rbuf[..num]));
                        }
                    }

                    AppState::Connected
                } else {
                    AppState::Scanning
                }
            }
        };
    }

    fn connected_update(&mut self) -> bool {
        true
    }

    fn scan<'a>(&'a mut self) -> Option<Device<'a>> {
        println!("Scanning!");
        for mut device in self.context.devices().unwrap().iter() {
            let device_desc = device.device_descriptor().unwrap();

            println!("Bux {:03} Device {:03} ID {:04x}:{:04x} configs: {}",
                device.bus_number(),
                device.address(),
                device_desc.vendor_id(),
                device_desc.product_id(),
                device_desc.num_configurations());
            if device_desc.vendor_id() == VENDOR_ID && device_desc.product_id() == PROD_ID {
                return Some(device);
            }
        }
        return None;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut app = App::new();
    loop {
        app.turn().await
    }
}


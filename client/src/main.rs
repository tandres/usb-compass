use libusb::{Context, Device, DeviceHandle};
use std::time::Duration;
use common::usb::{VENDOR_ID, PROD_ID};
use tokio::{
    sync::mpsc::{UnboundedSender, UnboundedReceiver},
    time::sleep,
};

mod error;

pub use error::{CompError, Result};

const DEFAULT_TIMEOUT: Duration = Duration::from_millis(500);
const WRITE_ADDR: u8 = 2;
const READ_ADDR: u8 = 130;

type Message = Vec<u8>;

fn scan<'a>(context: &'a Context) -> Result<Option<Device<'a>>> {
    println!("Scanning!");
    for device in context.devices()?.iter() {
        let device_desc = device.device_descriptor()?;

        println!("Bux {:03} Device {:03} ID {:04x}:{:04x} configs: {}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id(),
            device_desc.num_configurations());
        if device_desc.vendor_id() == VENDOR_ID && device_desc.product_id() == PROD_ID {
            println!("Found matching device!");
            return Ok(Some(device));
        }
    }
    return Ok(None);
}


fn configure<'a>(device: &Device<'a>, config_num: u8, claims: Vec<u8>) -> Result<DeviceHandle<'a>> {
    // Just sort of assuming now that this is our device. Maybe I will do more
    // with this later?
    let mut handle = device.open()?;
    if handle.kernel_driver_active(config_num)? {
        handle.detach_kernel_driver(config_num)?;
    }
    for claim in claims {
        handle.claim_interface(claim)?;
    }
    Ok(device.open()?)
}

fn print_device_info<'a>(device: &Device<'a>, print: bool) -> Result<()> {
    if !print {
        return Ok(());
    }
    let number_configs = device.device_descriptor()?.num_configurations();
    let active_config_descriptor = device.active_config_descriptor()?;
    let active_number = active_config_descriptor.number();
    println!("Device has {} configurations:", number_configs);
    for i in 0..number_configs {
        let config = device.config_descriptor(i)?;
        let config_number = config.number();
        let active_flag = if active_number == config_number {
            "*"
        } else {
            ""
        };
        println!("\tConfig {}{}:", config_number, active_flag);
        println!("\tInterfaces {}:", config.num_interfaces());
        for interface in config.interfaces() {
            println!("\t\tInterface no {}:", interface.number());
            for desc in interface.descriptors() {
                println!("\t\t\tDescriptor no {}", desc.interface_number());
                println!("\t\t\tEndpoints {}:", desc.num_endpoints());
                for endpoint in desc.endpoint_descriptors() {
                    println!("\t\t\t\t#{} @{} dir:{:?} type:{:?}",
                        endpoint.number(),
                        endpoint.address(),
                        endpoint.direction(),
                        endpoint.transfer_type());
                }
            }
        }
    }
    Ok(())
}

fn read_bulk<'a>(addr: u8, handle: &DeviceHandle<'a>) -> Result<Message> {
    let mut buf: [u8; 64] = [0; 64];
    let num = handle.read_bulk(addr, &mut buf, DEFAULT_TIMEOUT)?;
    Ok(buf[..num].to_vec())
}

fn write_bulk<'a>(addr: u8, buf: &[u8], handle: &DeviceHandle<'a>) -> Result<usize> {
    Ok(handle.write_bulk(addr, buf, DEFAULT_TIMEOUT)?)
}

async fn usb(mut to_board_rx: UnboundedReceiver<Message>, from_board_tx: UnboundedSender<Message>) -> Result<()> {
    let context = libusb::Context::new()?;

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Some(device) = scan(&context)? {
            print_device_info(&device, true)?;
            let handle = configure(&device, 1, vec![1])?;
            loop {
                if let Some(msg) = to_board_rx.recv().await {
                    println!("Sending: {:?}", msg);
                    if write_bulk(WRITE_ADDR, &msg, &handle).is_err() {
                        break;
                    }
                }
                match read_bulk(READ_ADDR, &handle) {
                    Ok(buf) => {
                        println!("Received: {:?}", buf);
                        from_board_tx.send(buf).unwrap();
                    }
                    Err(e) => {
                        println!("Error: {}", e);
                        break;
                    }
                }
            }
        } else {
            continue;
        }
    }
}

async fn chatter(to_board_tx: UnboundedSender<Message>, mut from_board_rx: UnboundedReceiver<Message>) {
    tokio::spawn(async move {
        loop {
            if let Some(val) = from_board_rx.recv().await {
                println!("Board said: {:?}", String::from_utf8_lossy(&val));
            }
        }
    });
    loop {
        sleep(Duration::from_secs(1)).await;
        let msg = b"hello!".to_vec();
        to_board_tx.send(msg).unwrap();
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (to_board_tx, to_board_rx) = tokio::sync::mpsc::unbounded_channel();
    let (from_board_tx, from_board_rx) = tokio::sync::mpsc::unbounded_channel();
    let usb = usb(to_board_rx, from_board_tx);
    let chatter = chatter(to_board_tx, from_board_rx);

    tokio::select! {
        _ = usb => {
            println!("Usb returned");
        }
        _ = chatter => {
            println!("Chatter returned");
        }
    }
}


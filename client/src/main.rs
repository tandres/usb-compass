use common::{
    message::Message,
    link::Link,
    usb::{VENDOR_ID, PROD_ID},
};
use log::*;
use rusb::{Device, DeviceHandle, UsbContext, Error as UsbError};
use std::time::Duration;
use tokio::{
    sync::broadcast::{channel, Sender, Receiver, error::TryRecvError},
    time::sleep,
};
mod error;

pub use error::{CompError, Result};

const WRITE_TIMEOUT: Duration = Duration::from_millis(10);
const READ_TIMEOUT: Duration = Duration::from_millis(10);
const WRITE_ADDR: u8 = 2;
const READ_ADDR: u8 = 130;
const DESIRED_CONFIG: u8 = 1;
const SERIAL_DATA_INTERFACE: u8 = 1;

fn print_device_info<T: UsbContext>(device: Device<T>) -> Result<()> {
    let number_configs = device.device_descriptor()?.num_configurations();
    let active_config_descriptor = device.active_config_descriptor()?;
    let active_number = active_config_descriptor.number();
    info!("Device has {} configurations:", number_configs);
    for i in 0..number_configs {
        let config = device.config_descriptor(i)?;
        let config_number = config.number();
        let active_flag = if active_number == config_number {
            "*"
        } else {
            ""
        };
        trace!("\tConfig {}{}:", config_number, active_flag);
        trace!("\tInterfaces {}:", config.num_interfaces());
        for interface in config.interfaces() {
            trace!("\t\tInterface no {}:", interface.number());
            for desc in interface.descriptors() {
                trace!("\t\t\tDescriptor no {}", desc.interface_number());
                trace!("\t\t\tEndpoints {}:", desc.num_endpoints());
                for endpoint in desc.endpoint_descriptors() {
                    trace!("\t\t\t\t#{} @{} dir:{:?} type:{:?}",
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

// fn read_bulk<'a>(addr: u8, handle: &DeviceHandle<'a>, link: &mut Link) -> Result<Message> {
//     let mut buf: [u8; 64] = [0; 64];
//     match handle.read_bulk(addr, &mut buf, DEFAULT_TIMEOUT)

// }

// fn write_bulk<'a>(addr: u8, buf: &[u8], handle: &DeviceHandle<'a>) -> Result<usize> {
//     Ok(handle.write_bulk(addr, buf, DEFAULT_TIMEOUT)?)
// }

fn usb_read<T: UsbContext>(handle: &mut DeviceHandle<T>, link: &mut Link) -> Result<Vec<Message>> {
    let mut buf = [0u8; Message::MAX_SIZE];
    let mut messages = Vec::new();
    let read = match handle.read_bulk(READ_ADDR, &mut buf, READ_TIMEOUT) {
        Ok(read) => read,
        Err(UsbError::Timeout) => return Ok(messages),
        Err(e) => Err(e)?,
    };
    let mut offset = 0;
    loop {
        let (size, rx) = link.decode(&buf[offset..read])?;
        if size == 0 {
            break;
        }
        if let Some(msg) = rx {
            messages.push(msg);
        }
        offset += size;
    }
    Ok(messages)
}

fn usb_write<T: UsbContext>(handle: &mut DeviceHandle<T>, msg: Message, link: &mut Link) -> Result<()> {
    let mut buf = [0u8; Message::MAX_SIZE];
    let size = link.encode(&msg, &mut buf)?;
    let write_size = handle.write_bulk(WRITE_ADDR, &buf[..size], WRITE_TIMEOUT)?;
    if size > write_size {
        log::warn!("Partial usb write!");
    }
    Ok(())
}

async fn usb_link<T: UsbContext>(
    to_board: &mut Receiver<Message>,
    from_board: &Sender<Message>,
    mut handle: DeviceHandle<T>,
) -> Result<()> {
    let mut link = Link::new();
    trace!("Starting usb link loop");
    loop {
        for msg in usb_read(&mut handle, &mut link)? {
            log::trace!("Received {:?}", msg);
            from_board.send(msg)?;
        }

        loop {
            match to_board.try_recv() {
                Ok(msg) => usb_write(&mut handle, msg, &mut link)?,
                Err(TryRecvError::Empty) => break,
                Err(e) => Err(e)?,
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn usb_configure<T: UsbContext>(handle: &mut DeviceHandle<T>) -> Result<()> {
    if DESIRED_CONFIG != handle.active_configuration()? {
        handle.set_active_configuration(DESIRED_CONFIG)?;
    }

    print_device_info(handle.device()).unwrap();
    if handle.kernel_driver_active(SERIAL_DATA_INTERFACE)? {
        handle.detach_kernel_driver(SERIAL_DATA_INTERFACE)?;
    }

    handle.claim_interface(SERIAL_DATA_INTERFACE)?;
    Ok(())
}

async fn usb(mut to_board_rx: Receiver<Message>, from_board_tx: Sender<Message>) -> Result<()> {
    let mut sleep_time = 1;
    loop {
        tokio::time::sleep(Duration::from_secs(sleep_time)).await;
        if let Some(mut handle) = rusb::open_device_with_vid_pid(VENDOR_ID, PROD_ID) {
            if let Err(e) = usb_configure(&mut handle) {
                error!("Failed to configure usb device! {}", e);
                sleep_time = 10;
                continue;
            }
            usb_link(&mut to_board_rx, &from_board_tx, handle).await?;
        }
    }
}


async fn chatter(to_board_tx: Sender<Message>, mut from_board_rx: Receiver<Message>) {
    tokio::spawn(async move {
        trace!("Starting receiver loop");
        loop {
            match from_board_rx.recv().await {
                Ok(msg) => trace!("Board said: {:?}", msg),
                Err(e) => error!("{:?}", e),
            }
        }
    });
    trace!("Starting chatter loop");
    loop {
        sleep(Duration::from_millis(500)).await;
        let msg = Message::MagReq;
        to_board_tx.send(msg).unwrap();
        let msg = Message::AccelReq;
        to_board_tx.send(msg).unwrap();
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();
    let (to_board_tx, to_board_rx) = channel(10);
    let (from_board_tx, from_board_rx) = channel(10);
    let usb = usb(to_board_rx, from_board_tx);
    let chatter = chatter(to_board_tx, from_board_rx);

    tokio::select! {
        res = usb => {
            println!("Usb returned: {:?}", res);
        }
        res = chatter => {
            println!("Chatter returned: {:?}", res);
        }
    }
}


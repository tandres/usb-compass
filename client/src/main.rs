use common::{
    message::Message,
    link::Link,
    usb::{VENDOR_ID, PROD_ID},
};
use log::{trace, info};
use rusb::{Device, DeviceHandle, UsbContext, Error as UsbError};
use std::time::Duration;
use std::thread::sleep;
use std::sync::{Arc, mpsc::{channel, Sender, Receiver, TryRecvError}, Mutex};
use bevy::{pbr::AmbientLight, prelude::*};

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

fn usb_link<T: UsbContext>(
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
        sleep(Duration::from_millis(50));
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

fn usb(mut to_board_rx: Receiver<Message>, from_board_tx: Sender<Message>) -> Result<()> {
    let mut sleep_time = 1;
    loop {
        sleep(Duration::from_secs(sleep_time));
        if let Some(mut handle) = rusb::open_device_with_vid_pid(VENDOR_ID, PROD_ID) {
            if let Err(e) = usb_configure(&mut handle) {
                error!("Failed to configure usb device! {}", e);
                sleep_time = 10;
                continue;
            }
            usb_link(&mut to_board_rx, &from_board_tx, handle)?;
        }
    }
}


fn chatter(to_board_tx: Sender<Message>, from_board_rx: Receiver<Message>, accel: Arc<Mutex<(f32, f32, f32)>>) {
    std::thread::spawn(move || {
        trace!("Starting receiver loop");
        loop {
            match from_board_rx.recv() {
                Ok(msg) => {
                    trace!("Board said: {:?}", msg);
                    if let Message::Accel(x, y, z) = msg {
                        let mut data = accel.lock().unwrap();
                        data.0 = x;
                        data.1 = y;
                        data.2 = z;
                    }

                }
                Err(e) => error!("{:?}", e),
            }
        }
    });
    trace!("Starting chatter loop");
    loop {
        sleep(Duration::from_millis(500));
        let msg = Message::MagReq;
        to_board_tx.send(msg).unwrap();
        let msg = Message::AccelReq;
        to_board_tx.send(msg).unwrap();
    }
}


fn main() {
    env_logger::init();
    let (to_board_tx, to_board_rx) = channel();
    let (from_board_tx, from_board_rx) = channel();
    std::thread::spawn( move || {
        usb(to_board_rx, from_board_tx).unwrap();
    });
    let accel = Arc::new(Mutex::new((0., 0., 0.)));
    let accel_clone = accel.clone();
    std::thread::spawn( move || {
        chatter(to_board_tx, from_board_rx, accel_clone);
    });
    App::build()
        .add_plugins(DefaultPlugins)
        .add_plugin(HelloPlugin)
        .insert_resource(AccelData(accel))
        .add_system(accel_system.system())
        .run();
}

pub struct HelloPlugin;

impl Plugin for HelloPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.insert_resource(AmbientLight {
                color: Color::WHITE,
                brightness: 1.0 / 5.0f32,
            })
            .add_startup_system(setup_scene.system())
            .add_system(rotator_system.system());
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // commands.spawn_bundle( PbrBundle {
    //     mesh: meshes.add(Mesh::from(shape::Box::new(4.0, 0.5, 2.0))),
    //     material: materials.add(Color::rgb(0.3, 0.5, 0.5).into()),
    //     transform: Transform::from_xyz(0.0, 0.5, 0.0),
    //     ..Default::default()
    // }).insert(Rotates);
    commands.spawn_bundle( PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Cube { size: 0.2 })),
        material: materials.add(Color::rgb(0.3, 0.0, 0.0).into()),
        transform: Transform::from_xyz(0.0, 0.5, 0.0),
        ..Default::default()
    }).insert(AccelVector);
    commands.spawn_bundle( LightBundle {
        transform: Transform::from_xyz(1.0, 1.0, 1.0),
        ..Default::default()
    });
    commands.spawn_bundle( PerspectiveCameraBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });

}

struct AccelData(Arc<Mutex<(f32, f32, f32)>>);

struct AccelVector;

fn accel_system(accel_data: Res<AccelData>, mut query: Query<&mut Transform, With<AccelVector>>) {
    let (x, y, z) = accel_data.0.lock().unwrap().clone();
    for mut transform in query.iter_mut() {
        *transform = Transform::from_xyz(x, y, z);
    }
}

struct Rotates;

fn rotator_system(time: Res<Time>, mut query: Query<&mut Transform, With<Rotates>>) {
    for mut transform in query.iter_mut() {
        *transform = Transform::from_rotation(Quat::from_rotation_y((4.0 * std::f32::consts::PI / 20.0) * time.delta_seconds(),)) * *transform;
    }
}

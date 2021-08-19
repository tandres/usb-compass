#![no_std]
#![no_main]

//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use stm32f3xx_hal as hal;

use common::{
    link::Link,
    usb::{VENDOR_ID, PROD_ID},
    Message,
};

use core::cell::RefCell;

use cortex_m::{asm::delay, interrupt::Mutex};
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

use accelerometer::{
    Accelerometer,
    vector::{F32x3, I16x3},
};
use stm32f3_discovery::compass::Compass;

use hal::{
    gpio::{gpioe, Output, PushPull},
    interrupt,
    pac,
    timer::{Timer, Event},
    usb::{Peripheral, UsbBus as UsbBusType},
};
use hal::prelude::*;

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

mod message_manager;

use message_manager::{message_pop, message_push};

type Timer7 = Timer<pac::TIM7>;
static TIMER7: Mutex<RefCell<Option<Timer7>>> = Mutex::new(RefCell::new(None));

type GreenLed = gpioe::PE15<Output<PushPull>>;
static GREENLED: Mutex<RefCell<Option<GreenLed>>> = Mutex::new(RefCell::new(None));

static TRIGGER_READ: Mutex<RefCell<bool>> = Mutex::new(RefCell::new(false));

struct App {
    mag_data: I16x3,
    accel_data: F32x3,
}

impl App {
    fn new() -> App {
        App {
            mag_data: I16x3::new(0, 0, 0),
            accel_data: F32x3::new(0., 0., 0.),
        }
    }
}

#[entry]
fn main() -> ! {
    let mut app = App::new();

    let peris = pac::Peripherals::take().unwrap();
    let mut acr = peris.FLASH.constrain().acr;
    let mut rcc = peris.RCC.constrain();

    let clocks = rcc.cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .pclk2(24.MHz())
        .freeze(&mut acr);

    let mut gpioa = peris.GPIOA.split(&mut rcc.ahb);
    let mut gpiob = peris.GPIOB.split(&mut rcc.ahb);
    let mut gpioe = peris.GPIOE.split(&mut rcc.ahb);

    let mut red_led = gpioe.pe13.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let mut orange_led = gpioe.pe14.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let green_led = gpioe.pe15.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);

    let mut usb_dp = gpioa
        .pa12
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    usb_dp.set_low().ok();
    delay(clocks.sysclk().0 / 100);

    let usb_dm = gpioa
        .pa11
        .into_af14_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
    let usb_dp = usb_dp
        .into_af14_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);

    let mut compass = Compass::new(
        gpiob.pb6,
        gpiob.pb7,
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
        peris.I2C1,
        clocks,
        &mut rcc.apb1,
    )
    .unwrap();

    let usb = Peripheral {
        usb: peris.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };
    let usb_bus = UsbBusType::new(usb);

    let mut serial = SerialPort::new(&usb_bus);

    let mut link = Link::new();
    // Thanks interbiometrics!
    let vid_pid = UsbVidPid(VENDOR_ID, PROD_ID);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, vid_pid)
        .manufacturer("Fake Company")
        .product("Serial Port")
        .serial_number("TEST")
        .device_class(USB_CLASS_CDC)
        .build();

    let mut tim7 = Timer::tim7(peris.TIM7, 1.Hz(), clocks, &mut rcc.apb1);
    tim7.listen(Event::Update);

    cortex_m::interrupt::free(|cs| {
        *TIMER7.borrow(cs).borrow_mut() = Some(tim7);
        *GREENLED.borrow(cs).borrow_mut() = Some(green_led);
        *TRIGGER_READ.borrow(cs).borrow_mut() = false;
    });

    unsafe { pac::NVIC::unmask(pac::Interrupt::TIM7) };
    message_manager::setup();
    let _ = hprintln!("Starting loop!");

    loop {
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }

        let mut buf = [0u8; 256];
        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                red_led.set_high().ok();
                decode_messages(&mut buf[..count], &mut link, &mut app);
            }
            _ => {}
        }
        red_led.set_low().ok();

        if let Some(msg) = message_pop() {
            encode_and_send(msg, &mut buf, &mut link, &mut serial);
        }
        let mut read = false;
        cortex_m::interrupt::free(|cs| {
            read = TRIGGER_READ.borrow(cs).replace(false);
        });
        if read {
            orange_led.set_high().ok();
            read_accel(&mut compass, &mut app);
            read_mag(&mut compass, &mut app);
            orange_led.set_low().ok();
        }
    }
}

fn read_accel(compass: &mut Compass, app: &mut App) {
    if let Ok(accel) = compass.accel_norm() {
        app.accel_data = accel;
    }
}

fn read_mag(compass: &mut Compass, app: &mut App) {
    if let Ok(mag) = compass.mag_raw() {
        app.mag_data = mag;
    }
}

fn encode_and_send<T: usb_device::bus::UsbBus>(
    msg: Message,
    buf: &mut [u8],
    link: &mut Link,
    serial: &mut SerialPort<T>
) {
    match link.encode(&msg, buf) {
        Ok(size) => {
            let mut write_offset = 0;
            while write_offset < size {
                match serial.write(&buf[write_offset..size]) {
                    Ok(len) if len > 0 => {
                        write_offset += len;
                    }
                    Ok(_) => {
                        break;
                    }
                    Err(e) => {
                        let _ = hprintln!("Partial write! {:?}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            let _ = hprintln!("Failed to encode! {:?}", e);
        }
    }

}

fn decode_messages(buf: &mut [u8], link: &mut Link, app: &mut App) {
    let length = buf.len();
    let mut offset = 0;
    loop {
        let read = match link.decode(&buf[offset..]) {
            Ok((read, Some(msg))) => {
                process_message(msg, app);
                read
            }
            Ok((read, None)) => {
                read
            }
            Err(e) => {
                let _ = hprintln!("Error decoding! {:?}", e);
                break;
            }
        };
        offset += read;
        if offset >= length {
            break;
        }
    }
}

fn process_message(msg: Message, app: &mut App) {
    use common::Message::*;
    match msg {
        Nop => (),
        Hello => {
            message_push(Message::HelloAck);
            ()
        }
        HelloAck => (),
        AccelReq => {
            let accel = &app.accel_data;
            let msg = Message::Accel(accel.x, accel.y, accel.z);
            message_push(msg);
        },
        MagReq => {
            let mag = &app.mag_data;
            let msg = Message::Mag(mag.x, mag.y, mag.z);
            message_push(msg);
        },
        _ => (),
    }
}

#[interrupt]
fn TIM7() {
    cortex_m::interrupt::free(|cs| {
        TIMER7.borrow(cs).borrow_mut().as_mut().unwrap().clear_update_interrupt_flag();
        GREENLED.borrow(cs).borrow_mut().as_mut().unwrap().toggle().unwrap();
        TRIGGER_READ.borrow(cs).replace(true);
    });
}


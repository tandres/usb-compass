#![no_std]
#![no_main]

//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use stm32f3xx_hal as hal;

use common::{
    link::Link,
    usb::{VENDOR_ID, PROD_ID},
};

use core::cell::RefCell;

use cortex_m::{asm::delay, interrupt::Mutex};
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

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

type Timer7 = Timer<pac::TIM7>;
static TIMER7: Mutex<RefCell<Option<Timer7>>> = Mutex::new(RefCell::new(None));

type GreenLed = gpioe::PE15<Output<PushPull>>;
static GREENLED: Mutex<RefCell<Option<GreenLed>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let peris = pac::Peripherals::take().unwrap();
    let mut acr = peris.FLASH.constrain().acr;
    let mut rcc = peris.RCC.constrain();

    let clocks = rcc.cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .pclk2(24.MHz())
        .freeze(&mut acr);

    let mut gpioe = peris.GPIOE.split(&mut rcc.ahb);
    let mut red_led = gpioe.pe13.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let green_led = gpioe.pe15.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);


    let mut gpioa = peris.GPIOA.split(&mut rcc.ahb);
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

    let usb = Peripheral {
        usb: peris.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };
    let usb_bus = UsbBusType::new(usb);

    let serial = SerialPort::new(&usb_bus);

    let mut link = Link::new(serial);
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
    });

    unsafe { pac::NVIC::unmask(pac::Interrupt::TIM7) };

    hprintln!("Starting loop!").unwrap();

    loop {
        if !usb_dev.poll(&mut [&mut link]) {
            continue;
        }

        let msg = link.try_recv().unwrap();
        hprintln!("{:?}", msg).unwrap();

        // let mut buf = [0u8, 64];

        // match serial.read(&mut buf) {
        //     Ok(count) if count > 0 => {
        //         red_led.set_high().ok();

        //         for c in buf[0..count].iter_mut() {
        //             if 0x61 <= *c && *c <= 0x7a {
        //                 *c &= !0x20;
        //             }
        //         }

        //         let mut write_offset = 0;
        //         while write_offset < count {
        //             match serial.write(&buf[write_offset..count]) {
        //                 Ok(len) if len > 0 => {
        //                     write_offset += len;
        //                 }
        //                 _ => {}
        //             }
        //         }
        //     }
        //     _ => {}
        // }
        // red_led.set_low().ok();
    }
}


#[interrupt]
fn TIM7() {
    cortex_m::interrupt::free(|cs| {
        TIMER7.borrow(cs).borrow_mut().as_mut().unwrap().clear_update_interrupt_flag();
        GREENLED.borrow(cs).borrow_mut().as_mut().unwrap().toggle().unwrap();
    });
}

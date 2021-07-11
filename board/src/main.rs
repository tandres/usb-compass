#![no_std]
#![no_main]

//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use stm32f3xx_hal as hal;

use core::cell::RefCell;

use cortex_m::{asm::delay, interrupt::Mutex};
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

use hal::{
    gpio::{gpioe, Output, PushPull},
    interrupt,
    pac,
    timer::{Timer, Event},
};
use hal::prelude::*;

type Timer7 = Timer<pac::TIM7>;
static TIMER7: Mutex<RefCell<Option<Timer7>>> = Mutex::new(RefCell::new(None));

type GreenLed = gpioe::PE15<Output<PushPull>>;
static GREENLED: Mutex<RefCell<Option<GreenLed>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let peris = pac::Peripherals::take().unwrap();
    let mut acr = peris.FLASH.constrain().acr;
    let mut rcc = peris.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut acr);

    let mut gpioe = peris.GPIOE.split(&mut rcc.ahb);
    let mut red_led = gpioe.pe13.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    let green_led = gpioe.pe15.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);

    let mut tim7 = Timer::tim7(peris.TIM7, 1.Hz(), clocks, &mut rcc.apb1);
    tim7.listen(Event::Update);

    cortex_m::interrupt::free(|cs| {
        *TIMER7.borrow(cs).borrow_mut() = Some(tim7);
        *GREENLED.borrow(cs).borrow_mut() = Some(green_led);
    });

    unsafe { pac::NVIC::unmask(pac::Interrupt::TIM7) };

    hprintln!("Starting loop!").unwrap();

    loop {
        red_led.toggle().unwrap();
        delay(8000000)
    }
}

#[interrupt]
fn TIM7() {
    cortex_m::interrupt::free(|cs| {
        TIMER7.borrow(cs).borrow_mut().as_mut().unwrap().clear_update_interrupt_flag();
        GREENLED.borrow(cs).borrow_mut().as_mut().unwrap().toggle().unwrap();
    });
}

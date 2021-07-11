#![no_std]
#![no_main]

//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use stm32f3xx_hal as hal;

use cortex_m::asm::delay;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

use hal::pac;
use hal::prelude::*;

#[entry]
fn main() -> ! {
    let peris = pac::Peripherals::take().unwrap();
    let mut rcc = peris.RCC.constrain();
    let mut gpioe = peris.GPIOE.split(&mut rcc.ahb);

    let mut red_led = gpioe.pe13.into_push_pull_output(&mut gpioe.moder, &mut gpioe.otyper);
    hprintln!("Hello!").unwrap();

    loop {
        red_led.toggle().unwrap();
        delay(8000000)
    }
}

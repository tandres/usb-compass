#![no_std]
#![no_main]

//use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;

#[entry]
fn main() -> ! {

    hprintln!("Hello!").unwrap();

    loop {
    }
}

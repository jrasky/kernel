#![no_std]
extern crate core as std;
extern crate kernel_std;
extern crate rlibc;
#[macro_use]
extern crate log;
extern crate serial;

use std::fmt::Display;

#[no_mangle]
pub extern "C" fn bootstrap() -> ! {
    // set up the serial line
    serial::setup_serial();

    // set up the reserve logger
    static RESERVE: &'static Fn(&log::Location, &Display, &Display) = &serial::reserve_log;
    log::set_reserve(Some(RESERVE));

    panic!("Hello from bootstrap!");
}


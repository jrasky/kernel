#![feature(const_fn)]
#![feature(alloc)]
#![feature(collections)]
#![cfg_attr(not(test), no_std)]
extern crate rlibc;
#[macro_use]
extern crate collections;
extern crate spin;
extern crate alloc;

use include::*;

use logger::Logger;

#[macro_use]
mod macros;
mod include;
mod logger;
mod point;

pub use logger::{Request, Location, Output};
pub use point::{trace, get_trace};

static LOGGER: RwLock<Logger> = RwLock::new(Logger::new());

pub fn level_name(level: usize) -> &'static str {
    match level {
        0 => "CRITICAL",
        1 => "ERROR",
        2 => "WARN",
        3 => "INFO",
        4 => "DEBUG",
        5 => "TRACE",
        _ => "",
    }
}

pub fn to_level(name: &str) -> Result<Option<usize>, ()> {
    match name {
        "any" | "ANY" => Ok(None),
        "critical" | "CRITICAL" => Ok(Some(0)),
        "error" | "ERROR" => Ok(Some(1)),
        "warn" | "WARN" => Ok(Some(2)),
        "info" | "INFO" => Ok(Some(3)),
        "debug" | "DEBUG" => Ok(Some(4)),
        "trace" | "TRACE" => Ok(Some(5)),
        _ => Err(())
    }
}

// TODO: set up different locks for these different operations

fn suppress<T>(callback: T) -> bool where T: FnOnce(&mut Logger) {
    static SUPPRESSED: AtomicUsize = AtomicUsize::new(0);
    static SUPPRESSED_INFO: AtomicBool = AtomicBool::new(false);

    if let Some(mut logger) = LOGGER.try_write() {
        callback(&mut logger);

        let count = SUPPRESSED.swap(0, Ordering::Relaxed);
        if count > 0 {
            if !SUPPRESSED_INFO.load(Ordering::Relaxed) {
                SUPPRESSED_INFO.store(true, Ordering::Relaxed);
                mem::drop(logger);
                warn!("At least {} log entries suppressed", count);
            } else {
                SUPPRESSED_INFO.store(false, Ordering::Relaxed);
            }
        }

        false
    } else {
        SUPPRESSED.fetch_add(1, Ordering::Relaxed);

        true
    }
}

pub fn set_output(output: Option<Box<Output>>) {
    suppress(|logger| logger.set_output(output));
}

pub fn log<T: Display, V: Display>(level: usize, location: &Location, target: V, message: T) {
    if suppress(|logger| logger.log(level, location, &target, &message)) && level == 0 {
        panic!("Suppressed {} {} at {}({}): {}", target, level_name(level), location.file, location.line, message);
    }
}

pub fn set_level(level: Option<usize>, filter: Option<&str>) {
    suppress(|logger| logger.set_level(level, filter));
}

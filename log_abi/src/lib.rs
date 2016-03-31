#![feature(raw)]
#![feature(const_fn)]
#![no_std]
extern crate core as std;
extern crate rlibc;
extern crate constants;
extern crate spin;

#[macro_use]
mod macros;
mod include;

use include::*;

static CALLBACK: Mutex<Option<TraitObject>> = Mutex::new(None);

pub struct Location {
    pub module_path: &'static str,
    pub file: &'static str,
    pub line: u32,
}

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

pub fn set_callback(func: &'static Fn(usize, &Location, &Display, &Display)) {
    unsafe {
        let trait_obj: TraitObject = mem::transmute(func);
        let mut callback = CALLBACK.lock();
        *callback = Some(trait_obj);
    }
}

unsafe fn get_callback() -> Option<&'static Fn(usize, &Location, &Display, &Display)> {
    if let Some(ref trait_ref) = *CALLBACK.lock() {
        Some(mem::transmute(trait_ref.clone()))
    } else {
        None
    }
}

pub fn log(level: usize, location: &Location, target: &Display, message: &Display) {
    static LOST: AtomicUsize = AtomicUsize::new(0);

    unsafe {
        if let Some(callback) = get_callback() {
            let lost = LOST.swap(0, Ordering::Relaxed);

            static LOCATION: Location = Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };

            if lost > 0 {
                callback(2, &LOCATION, &module_path!(),
                         &format_args!("Lost at least {} messages", lost));
            }

            callback(level, location, target, message);
        } else {
            LOST.fetch_add(1, Ordering::Relaxed);
        }
    }
}

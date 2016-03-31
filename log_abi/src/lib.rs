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
        }
    }
}

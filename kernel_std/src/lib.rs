#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(unwind_attributes)]
#![no_std]
extern crate core as std;
extern crate rlibc;
#[macro_use]
extern crate log;
extern crate memory;
extern crate serial;
extern crate alloc;

use std::sync::atomic::{Ordering, AtomicUsize};

use std::fmt;
use std::mem;

pub mod cpu;

pub struct BootInfo {
    command_line_size: u64,
    command_line: u64,
    memory_map_size: u64,
    memory_map: u64,
    initial_heap: u64
}

struct PanicInfo {
    msg: Option<fmt::Arguments<'static>>,
    file: &'static str,
    line: u32
}

pub fn early_setup() {
    // set up the serial line
    serial::setup_serial();

    // set up the reserve logger
    static RESERVE: &'static Fn(&log::Location, &Display, &Display) = &serial::reserve_log;
    log::set_reserve(Some(RESERVE));
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[lang = "panic_fmt"]
pub extern "C" fn kernel_panic(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    static PANIC_COUNT: AtomicUsize = AtomicUsize::new(0);
    static mut ORIG_PANIC: PanicInfo = PanicInfo {
        msg: None,
        file: "",
        line: 0
    };

    match PANIC_COUNT.fetch_add(1, Ordering::Relaxed) {
        0 => {
            unsafe {
                ORIG_PANIC = PanicInfo {
                    // ok to ignore lifetime because this function deviates
                    msg: Some(mem::transmute(msg)),
                    file: file,
                    line: line
                };
            }
            panic_fmt(msg, file, line)
        },
        1 => {
            unsafe {double_panic(&ORIG_PANIC, msg, file, line)}
        },
        2 => {
            triple_panic(file, line)
        },
        _ => {
            // give up
            panic_halt()
        }
    }
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn panic_fmt(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // enter reserve memory
    memory::enter_reserved();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::log(0, &loc, &module_path!(), &msg);

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn double_panic(original: &PanicInfo, msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // disable memory
    memory::disable();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::reserve_log(&loc, &module_path!(),
        &format_args!("Double panic: {}\nWhile processing panic at {}({}): {}",
                      msg, original.file, original.line,
                      original.msg.unwrap_or(format_args!("No message"))));

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn triple_panic(file: &'static str, line: u32) -> ! {
    // disable memory
    memory::disable();

    static LOCATION: log::Location = log::Location {
        module_path: module_path!(),
        file: file!(),
        line: line!()
    };

    log::reserve_log(&LOCATION, &module_path!(), &format_args!("Triple panic at {}({})", file, line));

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn panic_halt() -> ! {
    // clear interrupts and halt
    // processory must be reset to continue
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

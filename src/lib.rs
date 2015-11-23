#![feature(no_std, lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(core_str_ext)]
#![feature(reflect_marker)]
#![feature(asm)]
#![no_std]
extern crate rlibc;
extern crate spin;
#[macro_use]
extern crate log;

use core::ptr::*;
use core::fmt::Write;
use core::sync::atomic::{AtomicPtr, Ordering};
use core::fmt::{Debug, Display, Formatter};
use core::marker::Reflect;

use core::fmt;
use core::slice;
use core::str;

use spin::Mutex;

use log::{LogRecord, LogMetadata, SetLoggerError, LogLevelFilter, MaxLogLevelFilter};

mod logging;

static MEMORY: MemManager = MemManager {
    free: AtomicPtr::new(0 as *mut Allocation),
    used: AtomicPtr::new(0 as *mut Allocation),
};

struct Allocation {
    base: *mut u8,
    size: usize,
    next: AtomicPtr<Allocation>,
}

struct MemManager {
    free: AtomicPtr<Allocation>,
    used: AtomicPtr<Allocation>,
}

struct MBInfoMemTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

trait Error: Debug + Display + Reflect {
    fn description(&self) -> &str;

    fn cause(&self) -> Option<&Error> {
        None
    }
}

#[derive(Debug)]
enum MemError {
    InvalidRange,
}

impl Reflect for MemManager {}
unsafe impl Sync for MemManager {}

impl Display for MemError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "MemError: {}", self.description())
    }
}

impl Error for MemError {
    fn description(&self) -> &str {
        match self {
            &MemError::InvalidRange => "Invalid range",
        }
    }
}

impl Allocation {
    const fn new(base: *mut u8, size: usize) -> Allocation {
        Allocation {
            base: base,
            size: size,
            next: AtomicPtr::new(0 as *mut _),
        }
    }
}

unsafe fn parse_cmdline(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    let cmdline = match str::from_utf8(str_slice) {
        Ok(s) => {
            info!("Command line: {}", s);
            s
        }
        Err(e) => {
            warn!("Unable to decode boot command line: {}", e);
            return;
        }
    };
}

unsafe fn parse_bootloader(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    match str::from_utf8(str_slice) {
        Ok(s) => {
            info!("Booted from: {}", s);
        }
        Err(e) => {
            warn!("Unable to decode bootloader name: {}", e);
        }
    }
}

unsafe fn parse_memory(ptr: *const u32) {
    // memory map
    let entry_size = *ptr.offset(2).as_ref().unwrap();
    let mut entry_ptr = ptr.offset(4) as *const MBInfoMemTag;
    let entry_end = (entry_ptr as usize + *ptr.offset(1) as usize) as *const _;

    while entry_ptr < entry_end {
        let entry = entry_ptr.as_ref().unwrap();
        match entry.addr_type {
            1 => {
                info!("RAM: {:16x} - {:16x} available",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            3 => {
                info!("RAM: {:16x} - {:16x} ACPI",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            4 => {
                info!("RAM: {:16x} - {:16x} reserved, preserve",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            _ => {
                info!("RAM: {:16x} - {:16x} reserved",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
        }

        entry_ptr = align(entry_ptr as usize + entry_size as usize, 8) as *const _;
    }
}

unsafe fn parse_multiboot_tags(boot_info: *const u32) {
    // read multiboot info
    let mut ptr: *const u32 = boot_info;

    let total_size: u32 = *ptr.as_ref().unwrap();

    let end: *const u32 = (ptr as usize + total_size as usize) as *const _;

    ptr = align(ptr.offset(2) as usize, 8) as *const _;

    while ptr < end {
        trace!("Found multiboot info tag {}", *ptr.as_ref().unwrap());

        match *ptr.as_ref().unwrap() {
            0 => {
                trace!("End of tags");
                break;
            }
            1 => {
                parse_cmdline(ptr);
            }
            2 => {
                parse_bootloader(ptr);
            }
            6 => {
                parse_memory(ptr);
            }
            n => {
                // unknown tags aren't a huge issue
                debug!("Unknown tag {}", n);
            }
        }

        // advance to the next tag
        ptr = align(ptr as usize + *ptr.offset(1).as_ref().unwrap() as usize, 8) as *const _;
    }

}

#[no_mangle]
pub extern "C" fn kmain(boot_info: *const u32) -> ! {
    // kernel main

    // initialize logging
    if let Err(_) = logging::init_logging() {
        panic!("Failed to initialize logging");
    }

    info!("Hello!");

    trace!("debug info at: {:x}", boot_info as usize);

    unsafe {
        parse_multiboot_tags(boot_info);
    }
    
    unreachable!("kmain tried to return");
}

#[inline]
fn align(n: usize, to: usize) -> usize {
    (n + to - 1) & !(to - 1)
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    // no unwinding right now anyways
    unimplemented!();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    logging::write_line(format_args!("PANIC in {}, line {}: {}", file, line, msg));

    // loop clear interrupts and halt
    loop {
        unsafe {
            asm!("cli" :::: "volatile");
            asm!("hlt" :::: "volatile");
        }
    }
}

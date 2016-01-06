#![feature(lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(asm)]
#![no_std]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;

use core::fmt;
use core::slice;
use core::str;

use constants::*;

#[macro_use]
mod log;
mod error;
mod memory;
mod constants;

// pub use since they're exported
pub use memory::{__rust_allocate,
                 __rust_deallocate,
                 __rust_reallocate,
                 __rust_reallocate_inplace,
                 __rust_usable_size};

struct MBInfoMemTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

unsafe fn parse_cmdline(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    let cmdline = match str::from_utf8(str_slice) {
        Ok(s) => s,
        Err(e) => {
            warn!("Unable to decode boot command line: {}", e);
            return;
        }
    };

    info!("Command line: {}", cmdline);
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
                // register memory
                memory::exit_reserved();
                let base_addr = if entry.base_addr == 0 {
                    1
                } else {
                    entry.base_addr
                };
                memory::register(base_addr as *mut memory::Opaque, entry.length as usize);
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
            _ => {
                // unknown tags aren't a huge issue
            }
        }

        // advance to the next tag
        ptr = align(ptr as usize + *ptr.offset(1).as_ref().unwrap() as usize, 8) as *const _;
    }

}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main
    info!("Hello!");

    unsafe {
        parse_multiboot_tags(boot_info);
    }

    let mut x = vec![1, 2];
    x.push(3);
    x.push(4);

    info!("{:?}", x);

    unreachable!("kernel_main tried to return");
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
    // enter reserve memory
    memory::enter_reserved();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::log(0, &loc, module_path!(), msg);

    // clear interrupts and halt
    // processory must be reset to continue
    loop {
        unsafe {
            asm!("cli" :::: "volatile");
            asm!("hlt" :::: "volatile");
        }
    }
}

#![feature(lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![no_std]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate elfloader;

use collections::{Vec, String};

use elfloader::elf;

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

extern "C" {
    static _image_begin: u8;
    static _image_end: u8;
}

struct MBInfoMemTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

#[derive(Debug)]
struct MBElfSymTag {
    ty: u32,
    size: u32,
    num: u32,
    entsize: u32,
    shndx: u32
}

unsafe fn parse_elf(ptr: *const u32) {
    let info = (ptr as *const MBElfSymTag).as_ref().unwrap();

    let sections = slice::from_raw_parts((ptr as *const MBElfSymTag).offset(1) as *const elf::SectionHeader, info.num as usize);

    let mut sum: u64 = 0;

    for section in sections {
        if section.flags.0 & elf::SHF_ALLOC.0 == elf::SHF_ALLOC.0 {
            sum += section.size;
        }
    }

    info!("{:#x} bytes used by kernel", sum);
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

    let mut acc = format!("");
    let mut item: Option<String> = None;

    for ch in cmdline.chars() {
        match ch {
            '=' => {
                if acc == "log" {
                    item = Some(acc);
                }

                acc = format!("");
            },
            ' ' => {
                if let Some(ref item) = item {
                    parse_command(item, &acc);
                }

                item = None;
                acc.clear();
            },
            ch => {
                acc.push(ch);
            }
        }
    }

    if let Some(ref item) = item {
        parse_command(item, &acc);
    }
}

fn parse_command(item: &String, value: &String) {
    if item == "log" {
        if value == "any" || value == "ANY" {
            log::set_level(None);
        } else if value == "critical" || value == "CRITICAL" {
            log::set_level(Some(0));
        } else if value == "error" || value == "ERROR" {
            log::set_level(Some(1));
        } else if value == "warn" || value == "WARN" {
            log::set_level(Some(2));
        } else if value == "info" || value == "INFO" {
            log::set_level(Some(3));
        } else if value == "debug" || value == "DEBUG" {
            log::set_level(Some(4));
        } else if value == "trace" || value == "TRACE" {
            log::set_level(Some(5));
        } else {
            warn!("Unknown log level: {}", value);
        }
    }
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

    let image_begin = &_image_begin as *const _ as usize;
    let image_end = &_image_end as *const _ as usize;

    while entry_ptr < entry_end {
        let entry = entry_ptr.as_ref().unwrap();
        match entry.addr_type {
            1 => {
                info!("RAM: {:16x} - {:16x} available",
                      entry.base_addr,
                      entry.base_addr + entry.length);
                // register memory
                let base_addr = if entry.base_addr == 0 {
                    1
                } else {
                    entry.base_addr as usize
                };

                if image_begin <= base_addr
                    && base_addr <= image_end
                    && base_addr + entry.length as usize > image_end {
                    memory::register(image_end as *mut memory::Opaque,
                                     base_addr + entry.length as usize - image_end);
                } else if base_addr < image_begin
                    && image_end > base_addr + entry.length as usize
                    && base_addr + entry.length as usize > image_begin {
                    memory::register(image_begin as *mut memory::Opaque,
                                     base_addr - image_begin);
                } else if base_addr + (entry.length as usize) < image_begin {
                    memory::register(base_addr as *mut memory::Opaque, entry.length as usize);
                } else {
                    // do not register the section
                }
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
            9 => {
                parse_elf(ptr);
            }
            _ => {
                // unknown tags aren't a huge issue
                trace!("Found multiboot info tag {}", *ptr.as_ref().unwrap());
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

    trace!("Multiboot info at: {:#x}", boot_info as usize);

    unsafe {
        parse_multiboot_tags(boot_info);
    }

    trace!("Done parsing tags");

    // once we're done with multiboot info, we can safely exit reserve memory
    memory::exit_reserved();

    let mut x: Vec<usize> = vec![1, 2];
    x.push(3);
    x.push(4);

    info!("{:?}", x);

    unreachable!("kernel_main tried to return");
}

#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    // no unwinding right now anyways
    unimplemented!();
}

#[cold]
#[inline(never)]
#[no_mangle]
#[lang = "panic_fmt"]
pub extern "C" fn rust_begin_unwind(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
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

#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume(_: *mut memory::Opaque) {
    unimplemented!();
}

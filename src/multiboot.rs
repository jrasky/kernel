use collections::{String, Vec};

use core::slice;
use core::str;
use core::cmp;

use elfloader::elf;

use constants::*;
use memory::paging;
use log;
use memory;

extern "C" {
    static _image_begin: u8;
    static _image_end: u8;
    static _kernel_top: u8;
}

struct MemoryTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

#[derive(Debug)]
struct ElfSymbolTag {
    ty: u32,
    size: u32,
    num: u32,
    entsize: u32,
    shndx: u32,
}

pub unsafe fn parse_elf(ptr: *const u32) {
    let info = (ptr as *const ElfSymbolTag).as_ref().unwrap();

    let sections =
        slice::from_raw_parts((ptr as *const ElfSymbolTag).offset(1) as *const elf::SectionHeader,
                              info.num as usize);

    let mut sum: u64 = 0;

    for section in sections {
        if section.addr >= (&_kernel_top as *const u8 as u64) 
            && section.flags.0 & elf::SHF_ALLOC.0 == elf::SHF_ALLOC.0
        {
            // section is allocated
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
            }
            ' ' => {
                if let Some(ref item) = item {
                    parse_command(item, &acc);
                }

                item = None;
                acc.clear();
            }
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

unsafe fn parse_memory(ptr: *const u32) -> Vec<(*mut memory::Opaque, usize)> {
    // memory map
    let entry_size = *ptr.offset(2).as_ref().unwrap();
    let mut entry_ptr = ptr.offset(4) as *const MemoryTag;
    let entry_end = (entry_ptr as usize + *ptr.offset(1) as usize) as *const _;

    let image_begin = &_image_begin as *const _ as usize;
    let image_end = &_image_end as *const _ as usize;

    let mut memory_regions = vec![];

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

                if image_begin <= base_addr && base_addr <= image_end &&
                   base_addr + entry.length as usize > image_end {
                       memory::register(image_end as *mut memory::Opaque,
                                        base_addr + entry.length as usize - image_end);
                       memory_regions.push((image_end as *mut memory::Opaque,
                                            base_addr + entry.length as usize - image_end));
                } else if base_addr < image_begin && image_end > base_addr + entry.length as usize &&
                   base_addr + entry.length as usize > image_begin {
                    memory::register(image_begin as *mut memory::Opaque, base_addr - image_begin);
                       memory_regions.push((image_begin as *mut memory::Opaque, base_addr - image_begin));
                } else if base_addr + (entry.length as usize) < image_begin {
                    memory::register(base_addr as *mut memory::Opaque, entry.length as usize);
                       memory_regions.push((base_addr as *mut memory::Opaque, entry.length as usize));
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

    memory_regions
}

pub unsafe fn parse_multiboot_tags(boot_info: *const u32) -> Vec<(*mut memory::Opaque, usize)> {
    // read multiboot info
    let mut ptr: *const u32 = boot_info;

    let total_size: u32 = *ptr.as_ref().unwrap();

    let end: *const u32 = (ptr as usize + total_size as usize) as *const _;

    ptr = align(ptr.offset(2) as usize, 8) as *const _;

    let mut memory_regions = vec![];

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
                memory_regions = parse_memory(ptr);
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

    memory_regions
}

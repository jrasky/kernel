pub use c::boot_c_panic;

use std::ptr;
use std::mem;

use alloc::heap;
use alloc::boxed::Box;

use collections::{String, Vec};

use constants::*;

use kernel_std::BootInfo;

use paging::Region;

use log;

mod c {
    use std::ptr;
    use std::slice;
    use std::str;

    #[repr(C)]
    pub struct boot_info {
        command_line_size: usize,
        command_line: *const u8,
        memory_map_capacity: usize,
        memory_map_size: usize,
        memory_map: *const memory_region,
        modules_capacity: usize,
        modules_size: usize,
        modules: *const module
    }

    #[repr(C)]
    pub struct memory_region {
        start: u64,
        len: u64,
        ty: u32
    }

    #[repr(C)]
    pub struct module {
        start: u64,
        len: u64,
        cmdline_size: usize,
        cmdline: *const u8
    }

    extern "C" {
        static mut error_message: *const u8;

        fn parse_multiboot_info(info: *const (), kernel_info: *const boot_info) -> u32;
    }

    impl Drop for boot_info {
        fn drop(&mut self) {
            // if modules is not null, then it's allocated somewhere
            if !self.modules.is_null() {
                unsafe {heap::deallocate(self.modules as *mut _, self.modules_capacity * mem::size_of::<module>(),
                                         mem::size_of::<usize>())};
            }

            if !self.memory_map.is_null() {
                unsafe {heap::deallocate(self.memory_map as *mut _, self.memory_map_capacity * mem::size_of::<memory_region>(),
                                         mem::size_of::<usize>())};
            }
        }
    }

    #[inline(never)]
    #[cold]
    pub fn c_panic() -> ! {
        let mut size: isize = 0;

        while ptr::read(error_message.offset(size)) != 0 {
            size += 1;
        }

        let str_slice = slice::from_raw_parts(error_message, size as usize);

        match str::from_utf8(str_slice) {
            Ok(s) => {
                panic!("{}", s);
            }
            Err(e) => {
                panic!("C called panic with invalid string: {}", e);
            }
        }
    }
}

pub struct MemoryInfo {
    available: Vec<Region>,
    reserved: Vec<Region>,
    acpi: Vec<Region>,
    nvs: Vec<Region>,
    bad: Vec<Region>,
}

pub struct ModuleInfo {
    command_line: String,
    memory: Region
}

pub struct BootInfo {
    log_level: Option<usize>,
    memory: MemoryInfo,
    modules: Vec<ModuleInfo>
}

#[derive(Clone, Copy)]
enum CommandItem {
    LogLevel
}

fn parse_command_line(cmdline: &[u8]) -> Option<usize> {
    let line = match str::from_utf8(str_slice) {
        Ok(s) => s,
        Err(e) => {
            panic!("Command line was not valid utf8: {}", e);
        }
    };

    let mut log_level = None;

    let mut acc = String::new();
    let mut item = None;

    for ch in line.chars() {
        match ch {
            ' ' => {
                if let Some(item) = item {
                    match item {
                        CommandItem::LogLevel => {
                            if let Ok(level) = log::to_level(acc.as_ref()) {
                                log_level = level;
                            } else {
                                error!("Invalid log level: {}", acc);
                            }
                        }
                    }
                }

                // clear accumulator no matter what
                acc.clear();
            },
            '=' => {
                if acc == "log" {
                    item = Some(CommandItem::LogLevel);
                } else {
                    item = None;
                }

                // clear accumulator
                acc.clear();
            },
            ch => {
                // next character
                acc.push(ch);
            }
        }
    }

    log_level
}

fn parse_memory_info(memory: &[c::memory_region]) -> MemoryInfo {
    let mut info = MemoryInfo {
        available: vec![],
        reserved: vec![],
        acpi: vec![],
        nvs: vec![],
        bad: vec![]
    };

    for entry in memory.iter() {
        match entry.ty {
            MULTIBOOT_MEMORY_AVAILABLE => {
                info.available.push(Region::new(entry.start, entry.len));
            },
            MULTIBOOT_MEMORY_RESERVED => {
                info.reserved.push(Region::new(entry.start, entry.len));
            },
            MULTIBOOT_MEMORY_ACPI_RECLAIMABLE => {
                info.acpi.push(Region::new(entry.start, entry.len));
            },
            MULTIBOOT_MEMORY_NVS => {
                info.nvs.push(Region::new(entry.start, entry.len));
            },
            MULTIBOOT_MEMORY_BADRAM => {
                info.bad.push(Region::new(entry.start, entry.len));
            },
            ty => {
                unreachable!("Unknown memory type {}", ty);
            }
        }
    }

    info
}

fn parse_modules(modules: &[c::module]) -> Vec<ModuleInfo> {
    let mut module_info = vec![];

    for module in modules.iter() {
        let cmdline_slice = unsafe {slice::from_raw_parts(module.cmdline, module.cmdline_size)};
        match str::from_utf8(cmdline_slice) {
            Ok(s) => {
                module_info.push(ModuleInfo {
                    command_line: s.into(),
                    memory: Region::new(module.start, module.len)
                });
            },
            Err(e) => {
                panic!("Module command line was not utf-8: {}", e);
            }
        }
    }

    module_info
}

pub unsafe fn create_boot_info(multiboot_info: *const ()) -> BootInfo {
    let info = Box::into_raw(Box::new(c::boot_info {
        command_line_size: 0,
        command_line: ptr::null(),
        memory_map_size: 0,
        memory_map: ptr::null(),
        modules_size: 0,
        modules: ptr::null()
    }));

    if c::parse_multiboot_info(multiboot_info, info) != 0 {
        c::c_panic();
    }

    let info = Box::from_raw(info);

    if info.memory_map.is_null() {
        panic!("Did not get memory map in boot info");
    }

    if info.modules.is_null() {
        panic!("Did not get any modules in boot info");
    }

    let memory_info = parse_memory_info(slice::from_raw_parts(info.memory_map, info.memory_map_size));

    let module_info = parse_modules(slice::from_raw_parts(info.modules, info.modules_size));

    let log_level = if !info.command_line.is_null() {
        // parse command line
        parse_command_line(slice::from_raw_parts(info.comand_line, info.command_line_size))
    } else {
        None
    }

    BootInfo {
        log_level: log_level,
        memory: memory_info,
        modules: module_info
    }
}


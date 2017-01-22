#![feature(btree_range)]
#![feature(alloc)]
#![feature(collections)]
#![feature(stmt_expr_attributes)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(unwind_attributes)]
#![cfg_attr(feature = "freestanding", feature(shared))]
#![cfg_attr(feature = "freestanding", feature(unique))]
#![cfg_attr(feature = "freestanding", feature(heap_api))]
#![cfg_attr(feature = "freestanding", feature(drop_types_in_const))]
#![no_std]
extern crate core as std;
#[cfg(feature = "freestanding")]
extern crate rlibc;
extern crate constants;
#[macro_use]
extern crate log;
#[cfg(feature = "freestanding")]
extern crate memory;
extern crate serial;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate spin;

pub use allocator::{Region, Allocator};
pub use logging::{ReserveLogger};

use std::marker::PhantomData;
#[cfg(feature = "freestanding")]
use std::fmt::{Write};
use std::fmt::{Debug, Display};
#[cfg(feature = "freestanding")]
use std::sync::atomic::{AtomicUsize, Ordering};

use std::mem;
use std::slice;
use std::str;
use std::ptr;
#[cfg(feature = "freestanding")]
use std::fmt;

use collections::{String, Vec};

use constants::*;

#[cfg(feature = "freestanding")]
pub mod cpu;

mod allocator;
mod logging;

pub trait Error: Debug + Display {
    fn descripton(&self) -> &str;

    fn cause(&self) -> Option<&Error> {
        None
    }
}

#[derive(Debug)]
pub struct MemoryInfo {
    pub available: Vec<Region>,
    pub reserved: Vec<Region>,
    pub acpi: Vec<Region>,
    pub nvs: Vec<Region>,
    pub bad: Vec<Region>,
}

#[derive(Debug)]
pub struct ModuleInfo {
    pub command_line: String,
    pub memory: Region
}

#[derive(Debug)]
pub struct BootInfo {
    pub log_level: log::LogLevelFilter,
    pub memory: MemoryInfo,
    pub modules: Vec<ModuleInfo>
}

#[repr(packed)]
pub struct ModuleProto {
    command_line: BootSlice<u8>,
    memory: Region
}

#[repr(packed)]
pub struct MemoryProto {
    available: BootSlice<Region>,
    reserved: BootSlice<Region>,
    acpi: BootSlice<Region>,
    nvs: BootSlice<Region>,
    bad: BootSlice<Region>
}

#[repr(packed)]
pub struct BootProto {
    magic: u64,
    log_level: u64,
    optimistic_heap: u64,
    memory: MemoryProto,
    modules: BootSlice<ModuleProto>
}

#[repr(packed)]
struct BootSlice<T> {
    address: u64,
    size: u64,
    phantom: PhantomData<T>
}

#[cfg(feature = "freestanding")]
struct PanicInfo {
    msg: Option<fmt::Arguments<'static>>,
    file: &'static str,
    line: u32
}

#[cfg(feature = "freestanding")]
static mut LOGGER: Option<logging::MultiLogger> = None;

#[cfg(feature = "freestanding")]
pub fn early_setup() {
    // set up the serial line
    serial::setup_serial();

    // set up logging
    unsafe {
        assert!(log::set_logger_raw(|max_log_level| {
            LOGGER = Some(logging::MultiLogger::new(max_log_level));

            let handle = LOGGER.as_ref().unwrap();

            handle.set_max_level(log::LogLevelFilter::Trace);

            handle
        }).is_ok());
    }
}

#[cfg(all(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cfg(all(not(test), feature = "freestanding"))]
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

#[cfg(all(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
fn panic_fmt(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // enter reserve memory
    memory::enter_reserved();

    error!("PANIC at {}({}): {}", file, line, msg);

    panic_halt();
}

#[cfg(all(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
fn double_panic(original: &PanicInfo, msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // disable memory
    memory::disable();

    static mut RESERVE: ReserveLogger = ReserveLogger::new();

    unsafe {
        let _ = writeln!(RESERVE, "Double panic at {}({}): {}\nWhile processing panic at {}({}): {}",
                         file, line, msg,
                         original.file, original.line,
                         original.msg.unwrap_or(format_args!("No message")));
    }

    panic_halt();
}

#[cfg(all(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
fn triple_panic(file: &'static str, line: u32) -> ! {
    // just try to make some output
    static mut RESERVE: ReserveLogger = ReserveLogger::new();

    unsafe {
        let _ = writeln!(RESERVE, "Triple panic at {}({})", file, line);
    }

    panic_halt();
}

#[cfg(all(not(test), feature = "freestanding"))]
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

#[cfg(all(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

impl<T> BootSlice<T> {
    fn as_slice(&self) -> &'static [T] {
        unsafe {
            slice::from_raw_parts(self.address as *const T, self.size as usize)
        }
    }

    fn new(from: Vec<T>) -> BootSlice<T> {
        let slice = BootSlice {
            address: from.as_ptr() as u64,
            size: from.len() as u64,
            phantom: PhantomData
        };

        mem::forget(from);

        slice
    }
}

impl ModuleProto {
    pub fn command_line(&self) -> &'static str {
        let slice = self.command_line.as_slice();

        match str::from_utf8(slice) {
            Err(e) => {
                panic!("Module string was not utf-8: {}", e);
            },
            Ok(s) => {
                s
            }
        }
    }

    pub fn memory(&self) -> Region {
        self.memory
    }
}

impl MemoryProto {
    pub fn available(&self) -> &'static [Region] {
        self.available.as_slice()
    }

    pub fn reserved(&self) -> &'static [Region] {
        self.reserved.as_slice()
    }

    pub fn acpi(&self) -> &'static [Region] {
        self.acpi.as_slice()
    }

    pub fn nvs(&self) -> &'static [Region] {
        self.nvs.as_slice()
    }

    pub fn bad(&self) -> &'static [Region] {
        self.bad.as_slice()
    }
}

impl BootProto {
    pub fn create(info: BootInfo, optimistic_heap: u64) -> BootProto {
        let memory = MemoryProto {
            available: BootSlice::new(info.memory.available),
            reserved: BootSlice::new(info.memory.reserved),
            acpi: BootSlice::new(info.memory.acpi),
            nvs: BootSlice::new(info.memory.nvs),
            bad: BootSlice::new(info.memory.bad)
        };

        let mut modules_list = vec![];
        for module in info.modules {
            modules_list.push(ModuleProto {
                command_line: BootSlice::new(module.command_line.into_bytes()),
                memory: module.memory
            });
        }

        let modules = BootSlice::new(modules_list);

        BootProto {
            magic: BOOT_INFO_MAGIC,
            log_level: info.log_level as u64,
            optimistic_heap: optimistic_heap,
            memory: memory,
            modules: modules
        }
    }

    pub fn parse(address: u64) -> Result<BootProto, ()> {
        let info = unsafe {ptr::read(address as *const BootProto)};

        if info.magic != BOOT_INFO_MAGIC {
            Err(())
        } else {
            Ok(info)
        }
    }

    pub fn log_level(&self) -> log::LogLevelFilter {
        match self.log_level {
            0 => log::LogLevelFilter::Off,
            1 => log::LogLevelFilter::Error,
            2 => log::LogLevelFilter::Warn,
            3 => log::LogLevelFilter::Info,
            4 => log::LogLevelFilter::Debug,
            5 => log::LogLevelFilter::Trace,
            _ => unreachable!("LogLevelFilter was not valid")
        }
    }

    pub fn optimistic_heap(&self) -> u64 {
        self.optimistic_heap
    }

    pub fn memory(&self) -> &MemoryProto {
        &self.memory
    }

    pub fn modules(&self) -> &'static [ModuleProto] {
        self.modules.as_slice()
    }
}

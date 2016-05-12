#![feature(collections_bound)]
#![feature(btree_range)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unique)]
#![feature(heap_api)]
#![feature(reflect_marker)]
#![feature(shared)]
#![feature(stmt_expr_attributes)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(asm)]
#![feature(unwind_attributes)]
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

pub use allocator::{Region, Allocator};
pub use map::Map;

use include::*;

pub mod cpu;

mod include;
mod allocator;
mod map;

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
    pub log_level: Option<usize>,
    pub memory: MemoryInfo,
    pub modules: Vec<ModuleInfo>
}

pub struct ModuleProto {
    command_line: BootSlice<u8>,
    memory: Region
}

pub struct MemoryProto {
    available: BootSlice<Region>,
    reserved: BootSlice<Region>,
    acpi: BootSlice<Region>,
    nvs: BootSlice<Region>,
    bad: BootSlice<Region>
}

pub struct BootProto {
    magic: u64,
    log_level: Option<u64>,
    optimistic_heap: u64,
    memory: MemoryProto,
    modules: BootSlice<ModuleProto>
}

struct BootSlice<T> {
    address: u64,
    size: u64,
    phantom: PhantomData<T>
}

struct PanicInfo {
    msg: Option<fmt::Arguments<'static>>,
    file: &'static str,
    line: u32
}

#[cfg(feature = "freestanding")]
pub fn early_setup() {
    // set up the serial line
    serial::setup_serial();

    // set up the reserve logger
    static RESERVE: &'static Fn(&log::Location, &Display, &Display) = &serial::reserve_log;
    log::set_reserve(Some(RESERVE));
}

#[cfg(and(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cfg(and(not(test), feature = "freestanding"))]
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

#[cfg(and(not(test), feature = "freestanding"))]
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

#[cfg(and(not(test), feature = "freestanding"))]
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

#[cfg(and(not(test), feature = "freestanding"))]
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

#[cfg(and(not(test), feature = "freestanding"))]
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

#[cfg(and(not(test), feature = "freestanding"))]
#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
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
            log_level: info.log_level.map(|level| level as u64),
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

    pub fn log_level(&self) -> Option<usize> {
        self.log_level.map(|level| level as usize)
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

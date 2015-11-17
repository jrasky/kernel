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

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

static WRITER: Mutex<Writer> = Mutex::new(Writer::new(Color::LightGray, Color::Black));

// TODO: make this a queue instead
static LOGGER: LoggerWrapper = LoggerWrapper(Mutex::new(LoggerInner(Logger::Initial(VGALogger))));
static LOGGER_TRAIT: &'static log::Log = &LOGGER as &'static log::Log;

static MEMORY: MemManager = MemManager {
    free: AtomicPtr::new(0 as *mut Allocation),
    used: AtomicPtr::new(0 as *mut Allocation),
};

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Color {
    Black = 0,
    Blue,
    Green,
    Cyan,
    Red,
    Magenta,
    Brown,
    LightGray,
    DarkGray,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    Pink,
    Yellow,
    White,
}

#[derive(Clone, Copy)]
struct ColorCode(u8);

struct VGALogger;

struct LoggerWrapper(Mutex<LoggerInner>);

struct LoggerInner(Logger);

enum Logger {
    Initial(VGALogger),
    Filtered(KLogger),
}

struct KLogger {
    filter: MaxLogLevelFilter,
}

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

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: Unique<Buffer>,
    defered_newline: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
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

impl KLogger {
    pub fn set_filter(&mut self, level: LogLevelFilter) {
        self.filter.set(level);
    }
}

impl LoggerInner {
    pub fn set_filter(&mut self, level: LogLevelFilter) {
        if let Logger::Filtered(ref mut logger) = self.0 {
            logger.set_filter(level)
        } else {
            warn!("Log level tried to be set, but was not a Filtered logger");
        }
    }

    pub fn init_filtered(&mut self, filter: MaxLogLevelFilter) {
        if let Logger::Initial(_) = self.0 {
            self.0 = Logger::Filtered(KLogger { filter: filter });
        } else {
            panic!("init_filtered should only be called once");
        }
    }
}

impl LoggerWrapper {
    pub fn set_filter(&self, level: LogLevelFilter) {
        self.0.lock().set_filter(level);
    }

    pub fn init_filtered(&self, filter: MaxLogLevelFilter) {
        self.0.lock().init_filtered(filter);
    }
}

impl MemManager {
    /// Add a memory range to the manager
    ///
    /// &self so it works with static. Has side-effects, beware!
    fn add(&self, base: usize, size: usize) -> Result<(), MemError> {
        // create an Allocation at the base address
        let ptr = base as *mut Allocation;
        let alloc: &mut Allocation = try!(unsafe { ptr.as_mut() }.ok_or(MemError::InvalidRange));
        alloc.base = unsafe { ptr.offset(1) } as *mut _;
        alloc.size = size;
        Ok(())
    }

    /// Insert an Allocation into the free chain
    fn insert_free(&self, alloc: *mut Allocation) -> Result<(), MemError> {
        let last: *mut Allocation = self.free
                                        .compare_and_swap(0 as *mut _, alloc, Ordering::SeqCst);

        if last.is_null() {
            // first insert, we're done here
            return Ok(());
        }

        let alloc_ref = unsafe { alloc.as_ref() }.expect("Given bad allocation");

        // keep trying until we win
        loop {
            while !last.is_null() ||
                  unsafe { last.as_ref() }.expect("Memory corrupted").size > alloc_ref.size {

            }
        }
    }
}

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

impl Default for Writer {
    fn default() -> Writer {
        Writer::new(Color::LightGray, Color::Black)
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte)
        }
        Ok(())
    }
}

impl log::Log for VGALogger {
    fn enabled(&self, _: &LogMetadata) -> bool {
        true
    }

    #[allow(unused_must_use)]
    fn log(&self, record: &LogRecord) {
        WRITER.lock().write_fmt(format_args!("{}: {}\n", record.level(), record.args()));
    }
}

impl log::Log for KLogger {
    fn enabled(&self, _: &LogMetadata) -> bool {
        true
    }

    #[allow(unused_must_use)]
    fn log(&self, record: &LogRecord) {
        WRITER.lock().write_fmt(format_args!("{}: {}\n", record.level(), record.args()));
    }
}

impl log::Log for LoggerWrapper {
    fn enabled(&self, meta: &LogMetadata) -> bool {
        self.0.lock().enabled(meta)
    }

    fn log(&self, record: &LogRecord) {
        self.0.lock().log(record)
    }
}

impl log::Log for LoggerInner {
    fn enabled(&self, meta: &LogMetadata) -> bool {
        self.0.enabled(meta)
    }

    fn log(&self, record: &LogRecord) {
        self.0.log(record)
    }
}

impl log::Log for Logger {
    fn enabled(&self, meta: &LogMetadata) -> bool {
        use Logger::*;
        match self {
            &Initial(ref logger) => {
                logger.enabled(meta)
            }
            &Filtered(ref logger) => {
                logger.enabled(meta)
            }
        }
    }

    fn log(&self, record: &LogRecord) {
        use Logger::*;
        match self {
            &Initial(ref logger) => {
                logger.log(record)
            }
            &Filtered(ref logger) => {
                logger.log(record)
            }
        }
    }
}

impl Writer {
    pub const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe { Unique::new(0xb8000 as *mut _) },
            defered_newline: false,
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.request_new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }

                if self.defered_newline {
                    self.new_line();
                }

                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;

                self.buffer().chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };

                self.column_position += 1;
            }
        }
    }

    fn buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.get_mut() }
    }

    fn request_new_line(&mut self) {
        if self.defered_newline {
            self.new_line();
        }

        self.defered_newline = true;
    }

    fn new_line(&mut self) {
        {
            let buffer = self.buffer();

            for row in 0..(BUFFER_HEIGHT - 1) {
                buffer.chars[row] = buffer.chars[row + 1]
            }
        }

        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
        self.defered_newline = false;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        self.buffer().chars[row] = [blank; BUFFER_WIDTH];
    }
}

#[no_mangle]
pub extern "C" fn kmain(boot_info: *const u32) -> ! {
    // kernel main

    // initialize logging
    match init_logging() {
        Ok(_) => {
            trace!("Initialized logging");
        }
        Err(_) => {
            let _ = write!(WRITER.lock(), "Failed to initialize logging");
            panic!();
        }
    }

    info!("Hello!");

    trace!("debug info at: {:x}", boot_info as usize);

    unsafe {

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
                    let str_ptr = ptr.offset(2) as *const u8;
                    let mut size: isize = 0;

                    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
                        size += 1;
                    }

                    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

                    match str::from_utf8(str_slice) {
                        Ok(s) => {
                            info!("Command line: {}", s);
                        }
                        Err(e) => {
                            warn!("Unable to decode boot command line: {}", e);
                        }
                    }

                }
                2 => {
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
                6 => {
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
                _ => {
                    // do nothing
                }
            }

            // advance to the next tag
            ptr = align(ptr as usize + *ptr.offset(1).as_ref().unwrap() as usize, 8) as *const _;
        }
    }

    unreachable!("kmain tried to return");
}

fn init_logging() -> Result<(), SetLoggerError> {
    log::set_logger(|filter| {
        LOGGER.init_filtered(filter);
        LOGGER.set_filter(LogLevelFilter::max());
        &LOGGER_TRAIT as *const _
    })
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
    let _ = write!(WRITER.lock(), "PANIC in {}, line {}: {}", file, line, msg);

    // loop clear interrupts and halt
    loop {
        unsafe {
            asm!("cli" :::: "volatile");
            asm!("hlt" :::: "volatile");
        }
    }
}

#![feature(no_std, lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(core_str_ext)]
#![feature(asm)]
#![no_std]
extern crate rlibc;
extern crate spin;
#[macro_use]
extern crate log;

use core::ptr::*;
use core::fmt::Write;

use core::fmt;

use spin::Mutex;

use log::{LogRecord, LogMetadata, SetLoggerError, LogLevelFilter};

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

static WRITER: Mutex<Writer> = Mutex::new(
    Writer::new(Color::LightGray, Color::Black));

static LOGGER: VGALogger = VGALogger;
static LOGGER_TRAIT: &'static log::Log = &LOGGER as &'static log::Log;

#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Color {
    Black      = 0,
    Blue       = 1,
    Green      = 2,
    Cyan       = 3,
    Red        = 4,
    Magenta    = 5,
    Brown      = 6,
    LightGray  = 7,
    DarkGray   = 8,
    LightBlue  = 9,
    LightGreen = 10,
    LightCyan  = 11,
    LightRed   = 12,
    Pink       = 13,
    Yellow     = 14,
    White      = 15,
}

#[derive(Clone, Copy)]
struct ColorCode(u8);

struct VGALogger;

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT]
}

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: Unique<Buffer>,
    defered_newline: bool
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode
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

impl Writer {
    pub const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe {Unique::new(0xb8000 as *mut _)},
            defered_newline: false
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
                    color_code: self.color_code
                };

                self.column_position += 1;
            }
        }
    }
    
    fn buffer(&mut self) -> &mut Buffer {
        unsafe {self.buffer.get_mut()}
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
pub extern fn kmain() {
    // kernel main

    // initialize logging
    match init_logging() {
        Ok(_) => {
            trace!("Initialized logging");
        },
        Err(_) => {
            let _ = write!(WRITER.lock(), "Failed to initialize logging");
            panic!();
        }
    }

    info!("kmain reached");

    panic!("end of kmain");
}

fn init_logging() -> Result<(), SetLoggerError> {
    log::set_logger(|filter| {
        filter.set(LogLevelFilter::max());
        &LOGGER_TRAIT as *const _
    })
}

#[cfg(not(test))]
#[lang = "eh_personality"]
extern fn eh_personality() {}

#[cfg(not(test))]
#[cold] #[inline(never)]
#[lang = "panic_fmt"]
extern fn panic_fmt(msg: fmt::Arguments,
                    file: &'static str, line: u32) -> ! {
    let _ = write!(WRITER.lock(), "PANIC in {}, line {}: {}", file, line, msg);
    loop {
        unsafe {
            asm!("cli" :::: "volatile");
            asm!("hlt" :::: "volatile");
        }
    }
}

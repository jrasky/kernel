use core::ptr::*;

use core::fmt;

use core::fmt::{Write, Debug, Display, Formatter};

use log;

use spin::Mutex;

use log::{LogRecord, LogMetadata, SetLoggerError, LogLevelFilter, MaxLogLevelFilter};

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

static WRITER: Mutex<Writer> = Mutex::new(Writer::new(Color::LightGray, Color::Black));

// TODO: make this a queue instead
static LOGGER: LoggerWrapper = LoggerWrapper(Mutex::new(LoggerInner(Logger::Initial(VGALogger))));
static LOGGER_TRAIT: &'static log::Log = &LOGGER as &'static log::Log;

#[repr(u8)]
#[derive(Clone, Copy)]
enum Color {
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

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

struct Writer {
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

impl KLogger {
    fn set_filter(&mut self, level: LogLevelFilter) {
        self.filter.set(level);
    }
}

impl LoggerInner {
    fn set_filter(&mut self, level: LogLevelFilter) {
        if let Logger::Filtered(ref mut logger) = self.0 {
            logger.set_filter(level)
        } else {
            warn!("Log level tried to be set, but was not a Filtered logger");
        }
    }

    fn init_filtered(&mut self, filter: MaxLogLevelFilter) {
        if let Logger::Initial(_) = self.0 {
            self.0 = Logger::Filtered(KLogger { filter: filter });
        } else {
            panic!("init_filtered should only be called once");
        }
    }
}

impl LoggerWrapper {
    fn set_filter(&self, level: LogLevelFilter) {
        self.0.lock().set_filter(level);
    }

    fn init_filtered(&self, filter: MaxLogLevelFilter) {
        self.0.lock().init_filtered(filter);
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
        match self {
            &Logger::Initial(ref logger) => {
                logger.enabled(meta)
            }
            &Logger::Filtered(ref logger) => {
                logger.enabled(meta)
            }
        }
    }

    fn log(&self, record: &LogRecord) {
        match self {
            &Logger::Initial(ref logger) => {
                logger.log(record)
            }
            &Logger::Filtered(ref logger) => {
                logger.log(record)
            }
        }
    }
}

impl Writer {
    const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe { Unique::new(0xb8000 as *mut _) },
            defered_newline: false,
        }
    }

    fn write_byte(&mut self, byte: u8) {
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

// Public interface

/// Initializes logging
///
/// Sets up logging to use the VGALogger
pub fn init_logging() -> Result<(), SetLoggerError> {
    log::set_logger(|filter| {
        LOGGER.init_filtered(filter);
        LOGGER.set_filter(LogLevelFilter::max());
        &LOGGER_TRAIT as *const _
    })
}

/// Writes line to the VGA writer
///
/// Write a line directly to the VGA logger, ignoring any errors
#[allow(unused_must_use)]
pub fn write_line(args: fmt::Arguments) {
    WRITER.lock().write_fmt(args);
}

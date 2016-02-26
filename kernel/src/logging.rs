#[cfg(not(test))]
use core::fmt::Write;
#[cfg(test)]
use std::fmt::Write;

#[cfg(not(test))]
use core::fmt;
#[cfg(test)]
use std::fmt;

#[cfg(not(test))]
use core::ptr::Unique;
#[cfg(test)]
use std::ptr::Unique;

#[cfg(not(test))]
use core::fmt::Display;
#[cfg(test)]
use std::fmt::Display;

#[cfg(not(test))]
use core::cell::UnsafeCell;
#[cfg(test)]
use std::cell::UnsafeCell;

#[cfg(not(test))]
use collections::{Vec, String};

use spin::Mutex;

use log;

use constants::*;

#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // for completeness
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
    White
}

#[derive(Clone, Copy)]
struct ColorCode(u8);

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    #[cfg(not(test))]
    buffer: Unique<Buffer>,
    #[cfg(test)]
    buffer: Buffer,
    deferred_newline: bool,
    filters: Option<Vec<(String, Option<usize>)>>
}

struct Buffer {
    chars: [[ScreenChar; VGA_BUFFER_WIDTH]; VGA_BUFFER_HEIGHT],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

impl log::Output for Writer {
    fn log(&mut self, level: usize, location: &log::Location, target: &Display, message: &Display) {
        let target = format!("{}", target);

        // this is inefficient, but for speed just don't define infinite filters
        if let Some(ref filters) = self.filters {
            for &(ref filter, filter_level) in filters.iter() {
                if let Some(filter_level) = filter_level {
                    if target.as_str().starts_with(filter.as_str()) && level > filter_level {
                        // log entry is filtered out
                        return;
                    }
                }
            }
        }

        if level <= 1 {
            let _ = self.write_fmt(format_args!("{} {} at {}({}): {}\n", target, log::level_name(level), location.file, location.line, message));
        } else {
            let _ = self.write_fmt(format_args!("{} {}: {}\n", target, log::level_name(level), message));
        }
    }

    fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        if let Some(filter) = filter {
            if let Some(ref mut filters) = self.filters {
                filters.push((filter.into(), level));
            } else {
                self.filters = Some(vec![(filter.into(), level)]);
            }
        }
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

impl Writer {
    #[cfg(not(test))]
    pub const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe { Unique::new(VGA_BUFFER_ADDR as *mut _) },
            deferred_newline: false,
            filters: None
        }
    }

    #[cfg(test)]
    fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: Buffer {
                chars: [[ScreenChar {
                    ascii_character: 0,
                    color_code: ColorCode(0)
                }; VGA_BUFFER_WIDTH]; VGA_BUFFER_HEIGHT]
            },
            deferred_newline: false,
            filters: vec![]
        }
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.request_new_line(),
            byte => {
                if self.column_position >= VGA_BUFFER_WIDTH {
                    self.new_line();
                }

                if self.deferred_newline {
                    self.new_line();
                }

                let row = VGA_BUFFER_HEIGHT - 1;
                let col = self.column_position;

                self.get_buffer().chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code: self.color_code,
                };

                self.column_position += 1;
            }
        }
    }

    #[cfg(not(test))]
    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.get_mut() }
    }

    #[cfg(test)]
    fn get_buffer(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    fn request_new_line(&mut self) {
        if self.deferred_newline {
            self.new_line();
        }

        self.deferred_newline = true;
    }

    fn new_line(&mut self) {
        {
            let buffer = self.get_buffer();
            for row in 0..(VGA_BUFFER_HEIGHT - 1) {
                buffer.chars[row] = buffer.chars[row + 1];
            }
        }

        self.clear_row(VGA_BUFFER_HEIGHT - 1);
        self.column_position = 0;
        self.deferred_newline = false;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        self.get_buffer().chars[row] = [blank; VGA_BUFFER_WIDTH];
    }
}

struct ReserveWriter {
    inner: UnsafeCell<Writer>
}

unsafe impl Sync for ReserveWriter {}

#[allow(unused_must_use)]
pub fn reserve_log<T: Display>(message: T) {
    static WRITER: ReserveWriter = ReserveWriter {
        inner: UnsafeCell::new(Writer::new(Color::LightGray, Color::Black))
    };

    unsafe {
        writeln!(WRITER.inner.get().as_mut().unwrap(), "{}", message);
    }
}

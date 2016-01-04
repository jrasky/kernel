use core::fmt::Write;

use core::fmt;

use core::ptr::Unique;

use spin::Mutex;

use constants::*;

static WRITER: Mutex<Writer> = Mutex::new(Writer::new(Color::LightGray, Color::Black));

#[repr(u8)]
#[derive(Clone, Copy)]
#[allow(dead_code)] // for completeness
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
    White
}

#[derive(Clone, Copy)]
struct ColorCode(u8);

struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: Unique<Buffer>,
    deferred_newline: bool,
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

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

impl Writer {
    const fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe { Unique::new(VGA_BUFFER_ADDR as *mut _) },
            deferred_newline: false,
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

    fn get_buffer(&mut self) -> &mut Buffer {
        unsafe { self.buffer.get_mut() }
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

#[allow(unused_must_use)]
pub fn write_fmt(args: fmt::Arguments) {
    WRITER.lock().write_fmt(args);
}

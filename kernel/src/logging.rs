use include::*;

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
    inner: ReserveWriter,
    target_buffer: FixedString,
    filters: Vec<(String, Option<usize>)>
}

pub struct ReserveWriter {
    column_position: usize,
    color_code: ColorCode,
    #[cfg(not(test))]
    buffer: Unique<Buffer>,
    #[cfg(test)]
    buffer: Buffer,
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

struct FixedString {
    buffer: String,
    limit: usize
}

impl AsRef<str> for FixedString {
    fn as_ref(&self) -> &str {
        self.buffer.as_ref()
    }
}

impl From<FixedString> for String {
    fn from(item: FixedString) -> String {
        item.buffer
    }
}

impl Write for FixedString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for ch in s.chars() {
            try!(self.write_char(ch));
        }

        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        if self.buffer.len() + c.len_utf8() > self.limit {
            Err(fmt::Error)
        } else {
            self.buffer.push(c);
            Ok(())
        }
    }
}

impl FixedString {
    fn new() -> FixedString {
        FixedString {
            buffer: String::with_capacity(4),
            limit: 4
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.buffer.clear()
    }

    #[inline]
    fn get_limit(&self) -> usize {
        self.limit
    }

    fn set_limit(&mut self, limit: usize) {
        // maximum number of bytes for that number of characters
        self.limit = limit * 4;

        // reallocate if necessary
        let cap = self.buffer.capacity();
        if cap < self.limit {
            self.buffer.reserve(self.limit - cap);
        }
    }
}

impl ColorCode {
    const fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

impl log::Output for Writer {
    fn log(&mut self, level: usize, location: &log::Location, target: &Display, message: &Display) {
        // this is inefficient, but for speed just don't define infinite filters
        if !self.filters.is_empty() {
            // use a fixed-length buffer to avoid reallocation while logging output
            self.target_buffer.clear();
            
            // ignore result of write, because it may be too long
            let _ = write!(self.target_buffer, "{}", target);
            
            for &(ref filter, filter_level) in self.filters.iter() {
                if self.target_buffer.as_ref().starts_with(filter.as_str()) {
                    if let Some(filter_level) = filter_level {
                        if filter_level < level {
                            // log entry is filtered out
                            return;
                        }
                    } else {
                        // log entry is specifically included
                        break;
                    }
                }
            }
        }

        self.inner.log(level, location, target, message);
    }

    fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        if let Some(filter) = filter {
            if self.target_buffer.get_limit() < filter.len() {
                self.target_buffer.set_limit(filter.len());
            }

            self.filters.push((filter.into(), level));
        }
    }
}

impl log::Output for ReserveWriter {
    fn log(&mut self, level: usize, location: &log::Location, target: &Display, message: &Display) {
        if level <= 1 {
            let _ = writeln!(self, "{} {} at {}({}): {}", target, log::level_name(level), location.file, location.line, message);
            // print a trace
            let _ = log::write_trace(self);
            // and then a newline
            let _ = writeln!(self, "");
        } else {
            let _ = writeln!(self, "{} {}: {}", target, log::level_name(level), message);
        }
    }
}

impl Write for ReserveWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }
        Ok(())
    }
}

impl Writer {
    pub fn new(foreground: Color, background: Color) -> Writer {
        Writer {
            inner: ReserveWriter::new(foreground, background),
            target_buffer: FixedString::new(),
            filters: vec![]
        }
    }
}

impl ReserveWriter {
    #[cfg(not(test))]
    pub const fn new(foreground: Color, background: Color) -> ReserveWriter {
        ReserveWriter {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: unsafe { Unique::new(VGA_BUFFER_ADDR as *mut _) },
            deferred_newline: false
        }
    }

    #[cfg(test)]
    fn new(foreground: Color, background: Color) -> ReserveWriter {
        ReserveWriter {
            column_position: 0,
            color_code: ColorCode::new(foreground, background),
            buffer: Buffer {
                chars: [[ScreenChar {
                    ascii_character: 0,
                    color_code: ColorCode(0)
                }; VGA_BUFFER_WIDTH]; VGA_BUFFER_HEIGHT]
            },
            deferred_newline: false
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

#[cfg(not(test))]
#[allow(unused_must_use)]
pub fn reserve_log<T: Display>(message: T) {
    static mut WRITER: ReserveWriter = ReserveWriter::new(Color::LightGray, Color::Black);

    unsafe {
        writeln!(WRITER, "\n{}", message);
    }
}

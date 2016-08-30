use std::fmt::{Display, Write};

use collections::{Vec, String};

use std::fmt;

use log;

pub struct Logger<T: Write> {
    inner: T,
    target_buffer: FixedString,
    filters: Vec<(String, Option<usize>)>
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


impl<T: Write> log::Output for Logger<T> {
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

        if level <= 1 {
            let _ = writeln!(self.inner, "{} {} at {}({}): {}", target, log::level_name(level), location.file, location.line, message);
            // print a trace
            let _ = log::write_trace(&mut self.inner);
            // and then a newline
            let _ = writeln!(self.inner, "");
        } else {
            let _ = writeln!(self.inner, "{} {}: {}", target, log::level_name(level), message);
        }
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


impl<T: Write> Logger<T> {
    pub fn new(inner: T) -> Logger<T> {
        Logger {
            inner: inner,
            target_buffer: FixedString::new(),
            filters: vec![]
        }
    }
}

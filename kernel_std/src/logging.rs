use std::fmt::Write;

use std::fmt;

use alloc::boxed::Box;

use spin::RwLock;

use log;

use serial;

pub struct ReserveLogger;

pub struct MultiLogger {
    max_level: log::MaxLogLevelFilter,
    inner: RwLock<Option<Box<log::Log>>>
}

impl Write for ReserveLogger {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Ok(len) = serial::write(s.as_bytes()) {
            if len != s.as_bytes().len() {
                // error if not all bytes were written
                return Err(fmt::Error);
            }
        }

        Ok(())
    }
}

impl ReserveLogger {
    pub const fn new() -> ReserveLogger {
        ReserveLogger
    }
}

impl log::Log for ReserveLogger {
    fn enabled(&self, _: &log::LogMetadata) -> bool {
        true
    }

    fn log(&self, record: &log::LogRecord) {
        static mut LOGGER: ReserveLogger = ReserveLogger::new();

        unsafe {
            let _ = writeln!(
                LOGGER, "{} RESERVE at {}({}): {}", record.target(), 
                record.location().file(), record.location().line(),
                record.args());
        }
    }
}

impl MultiLogger {
    pub const fn new(max_level: log::MaxLogLevelFilter) -> MultiLogger {
        MultiLogger {
            max_level: max_level,
            inner: RwLock::new(None)
        }
    }

    pub fn set_logger(&self, logger: Box<log::Log>) {
        let mut inner = self.inner.write();

        *inner = Some(logger);
    }

    pub fn set_max_level(&self, level: log::LogLevelFilter) {
        self.max_level.set(level)
    }

    pub fn get_max_level(&self) -> log::LogLevelFilter {
        self.max_level.get()
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        static RESERVE: ReserveLogger = ReserveLogger::new();

        let inner = self.inner.read();

        if let Some(ref logger) = *inner {
            logger.enabled(metadata)
        } else {
            RESERVE.enabled(metadata)
        }
    }

    fn log(&self, record: &log::LogRecord) {
        static RESERVE: ReserveLogger = ReserveLogger::new();

        let inner = self.inner.read();

        if let Some(ref logger) = *inner {
            logger.log(record)
        } else {
            RESERVE.log(record)
        }
    }
}

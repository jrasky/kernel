use std::fmt::Write;

use collections::String;

use log;

use serial;

pub struct Logger {
    level: log::LogLevelFilter,
    filter: String
}

impl Logger {
    pub fn new(level: log::LogLevelFilter, filter: String) -> Logger {
        Logger {
            level: level,
            filter: filter
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::LogMetadata) -> bool {
        metadata.level() <= self.level && metadata.target().starts_with(&self.filter)
    }

    fn log(&self, record: &log::LogRecord) {
        if !self.enabled(record.metadata()) {
            // don't log anything disabled
            return;
        }

        if record.level() < log::LogLevel::Debug {
            assert!(writeln!(
                serial::Writer, "{} {}: {}",
                record.target(), record.level(), record.args()
            ).is_ok());
        } else {
            assert!(writeln!(
                serial::Writer, "{} {} at {}({}): {}",
                record.target(), record.level(),
                record.location().file(), record.location().line(),
                record.args()
            ).is_ok());
        }
    }
}

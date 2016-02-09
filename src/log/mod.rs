#[cfg(not(test))]
use core::fmt::Display;
#[cfg(test)]
use std::fmt::Display;

#[cfg(not(test))]
use core::fmt;
#[cfg(test)]
use std::fmt;

use collections::String;

use spin::Mutex;

mod vga;

#[macro_export]
macro_rules! log {
    (target: $target:expr, $level:expr, $($arg:tt)+) => (
        #[cfg(feature = "log_any")]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log($level, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($level:expr, $arg:tt)+) => (
        trace!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! critical {
    (target: $target:expr, $($arg:tt)+) => (
        // always log critical
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(0, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        critical!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn", feature = "log_error"))]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(1, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        error!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn"))]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(2, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        warn!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info"))]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(3, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        info!(target: module_path!(), $($arg)+)
    )
}


#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug"))]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(4, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        debug!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => (
        #[cfg(any(feature = "log_any", feature = "log_trace"))]
        {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log(5, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        trace!(target: module_path!(), $($arg)+)
    )
}

static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());

struct Logger {
    level: Option<usize>
}

pub struct Request {
    pub level: usize,
    pub location: Location,
    pub target: String,
    pub message: String
}

pub struct Location {
    pub module_path: &'static str,
    pub file: &'static str,
    pub line: u32
}

impl Logger {
    #[cfg(feature = "log_any")]
    const fn new() -> Logger {
        Logger {
            level: None
        }
    }

    #[cfg(not(any(feature = "log_any", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace")))]
    const fn new() -> Logger {
        Logger {
            level: Some(0)
        }
    }

    #[cfg(all(feature = "log_error", not(any(feature = "log_any", feature = "log_critical", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(1)
        }
    }

    #[cfg(all(feature = "log_warn", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_info", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(2)
        }
    }

    #[cfg(all(feature = "log_info", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(3)
        }
    }

    #[cfg(all(feature = "log_debug", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(4)
        }
    }

    #[cfg(all(feature = "log_trace", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(5)
        }
    }

    fn set_level(&mut self, level: Option<usize>) -> Option<usize> {
        let res = self.level;
        self.level = level;
        res
    }

    #[cfg(test)]
    fn log<T: Display, V: Display>(&mut self, level: usize, location: &Location,
                                   target: V, message: T) {
        // for testing, print everything
        println!("{} {} at {}({}): {}", target, self.level_name(level), location.file, location.line, message);
    }

    #[cfg(not(test))]
    fn log<T: Display, V: Display>(&mut self, level: usize, location: &Location,
                                   target: V, message: T) {
        // only one logger right now
        if let Some(log_level) = self.level {
            if level > log_level {
                // don't log
                return;
            }
        }

        // otherwise log
        if level <= 1 {
            vga::write_fmt(format_args!("{} {} at {}({}): {}\n", target, self.level_name(level), location.file, location.line, message));
        } else {
            vga::write_fmt(format_args!("{} {}: {}\n", target, self.level_name(level), message));
        }
    }

    fn reserve_log<T: Display, V: Display>(&mut self, level: usize, location: &Location,
                                           target: V, message: T) {
        // use vga logger as reserve
        vga::write_fmt(format_args!("{} {} at {}({}): {}\n", target, self.level_name(level), location.file, location.line, message));
    }

    fn level_name(&self, level: usize) -> &'static str {
        match level {
            0 => "CRITICAL",
            1 => "ERROR",
            2 => "WARN",
            3 => "INFO",
            4 => "DEBUG",
            5 => "TRACE",
            _ => ""
        }
    }
}

pub fn log<T: Display, V: Display>(level: usize, location: &Location, target: V, message: T) {
    LOGGER.lock().log(level, location, target, message)
}

pub fn reserve_log<T: Display, V: Display>(level: usize, location: &Location, target: V, message: T) {
    LOGGER.lock().log(level, location, target, message)
}

pub fn set_level(level: Option<usize>) -> Option<usize> {
    LOGGER.lock().set_level(level)
}

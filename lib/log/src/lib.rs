#![feature(const_fn)]
#![feature(alloc)]
#![feature(collections)]
#![cfg_attr(not(test), no_std)]
extern crate rlibc;
#[macro_use]
extern crate collections;
extern crate spin;
extern crate alloc;

#[cfg(not(test))]
use core::fmt::Display;
#[cfg(test)]
use std::fmt::Display;

#[cfg(not(test))]
use alloc::boxed::Box;
#[cfg(test)]
use std::boxed::Box;

#[cfg(test)]
use std::fmt;

use collections::String;

use spin::Mutex;

#[cfg(feature = "log_any")]
#[macro_export]
macro_rules! log {
    (target: $target:expr, $level:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log($level, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($level:expr, $arg:tt)+) => (
        log!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(feature = "log_any"))]
#[macro_export]
macro_rules! log {
    (target: $target:expr, $level:expr, $($arg:tt)+) => ();
    ($($level:expr, $arg:tt)+) => (
        log!(target: module_path!(), $($arg)+)
    )
}

#[macro_export]
macro_rules! critical {
    (target: $target:expr, $($arg:tt)+) => (
        // always log critical
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(0, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        critical!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn", feature = "log_error"))]
#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(1, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        error!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn", feature = "log_error")))]
#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        error!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn"))]
#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(2, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        warn!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn")))]
#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        warn!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info"))]
#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(3, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        info!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info")))]
#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        info!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(feature = "log_any", feature = "log_trace", feature = "log_debug"))]
#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(4, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        debug!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(any(feature = "log_any", feature = "log_trace", feature = "log_debug")))]
#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        debug!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(feature = "log_any", feature = "log_trace"))]
#[macro_export]
macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => (
        {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log(5, &LOCATION, $target, format_args!($($arg)+));
        }
    );
    ($($arg:tt)+) => (
        trace!(target: module_path!(), $($arg)+)
    )
}

#[cfg(not(any(feature = "log_any", feature = "log_trace")))]
#[macro_export]
macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        trace!(target: module_path!(), $($arg)+)
    )
}

static LOGGER: Mutex<Logger> = Mutex::new(Logger::new());

pub trait Output {
    fn log(&mut self, level: usize, location: &Location, target: &Display, message: &Display);
}

struct Logger {
    level: Option<usize>,
    output: Option<Box<Output>>
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
            level: None,
            output: None
        }
    }

    #[cfg(not(any(feature = "log_any", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace")))]
    const fn new() -> Logger {
        Logger {
            level: Some(0),
            output: None
        }
    }

    #[cfg(all(feature = "log_error", not(any(feature = "log_any", feature = "log_critical", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(1),
            output: None
        }
    }

    #[cfg(all(feature = "log_warn", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_info", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(2),
            output: None
        }
    }

    #[cfg(all(feature = "log_info", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_debug", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(3),
            output: None
        }
    }

    #[cfg(all(feature = "log_debug", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_trace"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(4),
            output: None
        }
    }

    #[cfg(all(feature = "log_trace", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug"))))]
    const fn new() -> Logger {
        Logger {
            level: Some(5),
            output: None
        }
    }

    fn set_output(&mut self, output: Option<Box<Output>>) {
        self.output = output;
    }

    fn set_level(&mut self, level: Option<usize>) -> Option<usize> {
        let res = self.level;
        self.level = level;
        res
    }

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
        if let Some(ref mut output) = self.output {
            output.log(level, location, &target, &message);
        }
    }
}

pub fn level_name(level: usize) -> &'static str {
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

pub fn set_output(output: Option<Box<Output>>) {
    LOGGER.lock().set_output(output)
}

pub fn log<T: Display, V: Display>(level: usize, location: &Location, target: V, message: T) {
    LOGGER.lock().log(level, location, target, message)
}

pub fn set_level(level: Option<usize>) -> Option<usize> {
    LOGGER.lock().set_level(level)
}

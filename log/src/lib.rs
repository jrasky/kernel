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

#[cfg(not(test))]
use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

#[cfg(test)]
use std::fmt;

#[cfg(not(test))]
use core::mem;
#[cfg(test)]
use std::mem;

use collections::String;

use spin::RwLock;

#[cfg(any(all(feature = "log_any", debug_assertions), feature = "release_log_any"))]
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

#[cfg(not(any(all(feature = "log_any", debug_assertions), feature = "release_log_any")))]
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

#[cfg(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn", feature = "log_error"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info", feature = "release_log_warn", feature = "log_error"), not(debug_assertions))))]
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

#[cfg(not(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn", feature = "log_error"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info", feature = "release_log_warn", feature = "log_error"), not(debug_assertions)))))]
#[macro_export]
macro_rules! error {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        error!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info", feature = "release_log_warn"), not(debug_assertions))))]
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

#[cfg(not(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info", feature = "log_warn"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info", feature = "release_log_warn"), not(debug_assertions)))))]
#[macro_export]
macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        warn!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info"), not(debug_assertions))))]
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

#[cfg(not(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug", feature = "log_info"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug", feature = "release_log_info"), not(debug_assertions)))))]
#[macro_export]
macro_rules! info {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        info!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug"), not(debug_assertions))))]
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

#[cfg(not(any(all(any(feature = "log_any", feature = "log_trace", feature = "log_debug"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace", feature = "release_log_debug"), not(debug_assertions)))))]
#[macro_export]
macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        debug!(target: module_path!(), $($arg)+)
    )
}

#[cfg(any(all(any(feature = "log_any", feature = "log_trace"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace"), not(debug_assertions))))]
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

#[cfg(not(any(all(any(feature = "log_any", feature = "log_trace"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_trace"), not(debug_assertions)))))]
#[macro_export]
macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => ();
    ($($arg:tt)+) => (
        trace!(target: module_path!(), $($arg)+)
    )
}

static LOGGER: RwLock<Logger> = RwLock::new(Logger::new());

pub trait Output {
    fn log(&mut self, level: usize, location: &Location, target: &Display, message: &Display);

    fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        // silence warnings
        let _ = level;
        let _ = filter;
    }
}

struct Logger {
    level: Option<usize>,
    output: Option<Box<Output>>,
}

pub struct Request {
    pub level: usize,
    pub location: Location,
    pub target: String,
    pub message: String,
}

pub struct Location {
    pub module_path: &'static str,
    pub file: &'static str,
    pub line: u32,
}

impl Logger {
    #[cfg(any(all(feature = "log_any", debug_assertions), all(feature = "release_log_any", not(debug_assertions))))]
    const fn new() -> Logger {
        Logger {
            level: None,
            output: None,
        }
    }

    #[cfg(not(any(all(any(feature = "log_any", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace"), not(debug_assertions)))))]
    const fn new() -> Logger {
        Logger {
            level: Some(0),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_error", not(any(feature = "log_any", feature = "log_critical", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_error", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    const fn new() -> Logger {
        Logger {
            level: Some(1),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_warn", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_warn", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    const fn new() -> Logger {
        Logger {
            level: Some(2),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_info", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_info", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_debug", feature = "release_log_trace")))))]
    const fn new() -> Logger {
        Logger {
            level: Some(3),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_debug", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_trace"))), all(feature = "release_log_debug", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_trace")))))]
    const fn new() -> Logger {
        Logger {
            level: Some(4),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_trace", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug"))), all(feature = "release_log_trace", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug")))))]
    const fn new() -> Logger {
        Logger {
            level: Some(5),
            output: None,
        }
    }

    fn set_output(&mut self, output: Option<Box<Output>>) {
        self.output = output;
    }

    fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        if filter.is_none() {
            self.level = level;
        }

        if let Some(ref mut output) = self.output {
            output.set_level(level, filter);
        }
    }

    fn log<T: Display, V: Display>(&mut self,
                                   level: usize,
                                   location: &Location,
                                   target: V,
                                   message: T) {
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
        _ => "",
    }
}

pub fn to_level(name: &str) -> Result<Option<usize>, ()> {
    match name {
        "any" | "ANY" => Ok(None),
        "critical" | "CRITICAL" => Ok(Some(0)),
        "error" | "ERROR" => Ok(Some(1)),
        "warn" | "WARN" => Ok(Some(2)),
        "info" | "INFO" => Ok(Some(3)),
        "debug" | "DEBUG" => Ok(Some(4)),
        "trace" | "TRACE" => Ok(Some(5)),
        _ => Err(())
    }
}

// TODO: set up different locks for these different operations

pub fn set_output(output: Option<Box<Output>>) {
    if let Some(mut logger) = LOGGER.try_write() {
        logger.set_output(output);
    } else {
        panic!("Tried to set output while logging");
    }
}

pub fn log<T: Display, V: Display>(level: usize, location: &Location, target: V, message: T) {
    static SUPPRESSED: AtomicUsize = AtomicUsize::new(0);
    static SUPPRESSED_INFO: AtomicBool = AtomicBool::new(false);

    if let Some(mut logger) = LOGGER.try_write() {
        logger.log(level, location, target, message);

        let count = SUPPRESSED.swap(0, Ordering::Relaxed);
        if count > 0 {
            if !SUPPRESSED_INFO.load(Ordering::Relaxed) {
                SUPPRESSED_INFO.store(true, Ordering::Relaxed);
                mem::drop(logger);
                warn!("At least {} log entries suppressed", count);
            } else {
                SUPPRESSED_INFO.store(false, Ordering::Relaxed);
            }
        }
    } else {
        SUPPRESSED.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn set_level(level: Option<usize>, filter: Option<&str>) {
    if let Some(mut logger) = LOGGER.try_write() {
        logger.set_level(level, filter);
    } else {
        panic!("Tried to set level while logging");
    }
}

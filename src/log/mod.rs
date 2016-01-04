use core::fmt::Display;

use spin::Mutex;

mod vga;

static LOGGER: Mutex<Logger> = Mutex::new(Logger);

struct Logger;

pub struct Location {
    pub module_path: &'static str,
    pub file: &'static str,
    pub line: u32
}

impl Logger {
    fn log<T: Display, V: Display>(&mut self, level: usize, _: &Location,
                                   target: V, message: T) {
        // only one logger right now
        vga::write_fmt(format_args!("{} {}: {}\n", target, self.level_name(level), message));
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

#[macro_export]
macro_rules! log {
    (target: $target:expr, $level:expr, $($arg:tt)+) => ({
        if cfg!(feature = "log_any") {
            static LOCATION: $crate::log::Location = $crate::log::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::log::log($level, &LOCATION, $target, format_args!($($arg)+));
        }
    });
    ($level:expr, $($arg:tt)+) => (log!(target: module_path!(), $level, $($arg)+))
}

#[macro_export]
macro_rules! critical {
    (target: $target:expr, $($arg:tt)+) => (
        if cfg!(feature = "log_critical") || cfg!(feature = "log_any") {
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
        if cfg!(feature = "log_error") || cfg!(feature = "log_any") {
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
        if cfg!(feature = "log_warn") || cfg!(feature = "log_any") {
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
        if cfg!(feature = "log_info") || cfg!(feature = "log_any") {
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
        if cfg!(feature = "log_debug") || cfg!(feature = "log_any") {
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
        if cfg!(feature = "log_trace") || cfg!(feature = "log_any") {
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

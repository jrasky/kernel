use include::*;

pub trait Output {
    fn log(&mut self, level: usize, location: &Location, target: &Display, message: &Display);

    fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        // silence warnings
        let _ = level;
        let _ = filter;
    }
}

pub struct Logger {
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
    pub const fn new() -> Logger {
        Logger {
            level: None,
            output: None,
        }
    }

    #[cfg(not(any(all(any(feature = "log_any", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace"), not(debug_assertions)))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(0),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_error", not(any(feature = "log_any", feature = "log_critical", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_error", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(1),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_warn", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_warn", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(2),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_info", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_info", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(3),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_debug", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_trace"))), all(feature = "release_log_debug", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(4),
            output: None,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_trace", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug"))), all(feature = "release_log_trace", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(5),
            output: None,
        }
    }

    pub fn set_output(&mut self, output: Option<Box<Output>>) {
        self.output = output;
    }

    pub fn set_level(&mut self, level: Option<usize>, filter: Option<&str>) {
        if filter.is_none() {
            self.level = level;
        }

        if let Some(ref mut output) = self.output {
            output.set_level(level, filter);
        }
    }

    pub fn log<T: Display, V: Display>(&mut self,
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

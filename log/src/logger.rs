use include::*;

use log_abi::Location;
use log_abi;

static LOGGER: RwLock<Logger> = RwLock::new(Logger::new());

pub struct Request {
    pub level: usize,
    pub location: Location,
    pub target: String,
    pub message: String,
}

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
    reserve: Option<&'static Fn(&Location, &Display, &Display)>,
    lost: usize
}

impl Logger {
    #[cfg(any(all(feature = "log_any", debug_assertions), all(feature = "release_log_any", not(debug_assertions))))]
    pub const fn new() -> Logger {
        Logger {
            level: None,
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    #[cfg(not(any(all(any(feature = "log_any", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"), debug_assertions), all(any(feature = "release_log_any", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace"), not(debug_assertions)))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(0),
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_error", not(any(feature = "log_any", feature = "log_critical", feature = "log_warn", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_error", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(1),
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_warn", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_info", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_warn", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_info", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(2),
            output: None,
            reserve: None,
            lost: 0
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_info", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_debug", feature = "log_trace"))), all(feature = "release_log_info", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_debug", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(3),
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_debug", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_trace"))), all(feature = "release_log_debug", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_trace")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(4),
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    #[cfg(any(all(debug_assertions, feature = "log_trace", not(any(feature = "log_any", feature = "log_critical", feature = "log_error", feature = "log_warn", feature = "log_info", feature = "log_debug"))), all(feature = "release_log_trace", not(debug_assertions), not(any(feature = "release_log_any", feature = "release_log_critical", feature = "release_log_error", feature = "release_log_warn", feature = "release_log_info", feature = "release_log_debug")))))]
    pub const fn new() -> Logger {
        Logger {
            level: Some(5),
            output: None,
            reserve: None,
            lost: 0,
        }
    }

    pub fn has_output(&self) -> bool {
        self.output.is_some()
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

    pub fn set_reserve(&mut self, output: Option<&'static Fn(&Location, &Display, &Display)>) {
        self.reserve = output;
    }

    pub fn reserve_log(&mut self, location: &Location, target: &Display, message: &Display) {
        if let Some(ref output) = self.reserve {
            if self.lost > 0 {
                static LOCATION: Location = Location {
                    module_path: module_path!(),
                    file: file!(),
                    line: line!()
                };

                output(&LOCATION, &module_path!(), &format_args!("Lost at least {} messages", self.lost));
                self.lost = 0;
            }

            output(location, target, message)
        } else {
            self.lost += 1;
        }
    }

    pub fn log(&mut self, level: usize, location: &Location, target: &Display, message: &Display) {
        // only one logger right now
        if let Some(log_level) = self.level {
            if level > log_level {
                // don't log
                return;
            }
        }

        // otherwise log
        if let Some(ref mut output) = self.output {
            if self.lost > 0 {
                // log a warning about lost messages
                let loc = Location {
                    module_path: module_path!(),
                    file: file!(),
                    line: line!()
                };

                output.log(2, &loc, &module_path!(), &format_args!("Lost at least {} messages", self.lost));
                self.lost = 0;
            }
            
            // then log the message
            output.log(level, location, target, message);
        } else {
            self.reserve_log(location, target, message);
        }
    }
}

fn suppress<T>(callback: T) -> bool where T: FnOnce(&mut Logger) {
    static SUPPRESSED: AtomicUsize = AtomicUsize::new(0);
    static SUPPRESSED_INFO: AtomicBool = AtomicBool::new(false);

    if let Some(mut logger) = LOGGER.try_write() {
        callback(&mut logger);

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

        false
    } else {
        SUPPRESSED.fetch_add(1, Ordering::Relaxed);

        true
    }
}

pub fn has_output() -> bool {
    LOGGER.read().has_output()
}

fn set_callback() {
    static REF: &'static (Fn(usize, &Location, &Display, &Display) + Send + Sync) = &log;
    log_abi::set_callback(REF);
}

pub fn set_output(output: Option<Box<Output>>) {
    suppress(|logger| logger.set_output(output));
    set_callback();
}

pub fn set_reserve(output: Option<&'static Fn(&Location, &Display, &Display)>) {
    suppress(|logger| logger.set_reserve(output));
    set_callback();
}

pub fn reserve_log(location: &Location, target: &Display, message: &Display) {
    suppress(|logger| logger.reserve_log(location, target, message));
}

pub fn log(level: usize, location: &Location, target: &Display, message: &Display) {
    if suppress(|logger| logger.log(level, location, &target, &message)) && level == 0 {
        panic!("Suppressed {} {} at {}({}): {}", target, log_abi::level_name(level), location.file, location.line, message);
    }
}

pub fn set_level(level: Option<usize>, filter: Option<&str>) {
    suppress(|logger| logger.set_level(level, filter));
}

#[cfg(any(debug_assertions, feature = "release_trace_points"))]
#[macro_export]
macro_rules! frame {
    ($name:ident) => {
        let mut $name = $crate::PointFrame::new();
    }
}

#[cfg(not(any(debug_assertions, feature = "release_trace_points")))]
#[macro_export]
macro_rules! frame {
    ($name:ident) => ()
}

#[cfg(any(debug_assertions, feature = "release_trace_points"))]
#[macro_export]
macro_rules! point {
    ($into:ident, $($arg:tt)+) => ({
        static LOCATION: $crate::Location = $crate::Location {
            module_path: module_path!(),
            file: file!(),
            line: line!()
        };
        (&mut $into as &mut $crate::Frame).add($crate::trace(&LOCATION, format_args!($($arg)+)));
    });
}

#[cfg(not(any(debug_assertions, feature = "release_trace_points")))]
#[macro_export]
macro_rules! point {
    ($into:ident, $($arg:tt)+) => ();
}

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

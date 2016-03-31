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

#[cfg(any(debug_assertions, feature = "release_trace_points"))]
#[macro_export]
macro_rules! frame {
    ($name:ident) => {
        let mut $name = $crate::PointFrame::new();
    };
    ($name:ident, $($arg:tt)+) => {
        let mut $name = {
            static LOCATION: $crate::Location = $crate::Location {
                module_path: module_path!(),
                file: file!(),
                line: line!()
            };
            $crate::PointFrame::new_add(&LOCATION, &format_args!($($arg)+))
        };
    }
}

#[cfg(not(any(debug_assertions, feature = "release_trace_points")))]
#[macro_export]
macro_rules! frame {
    ($name:ident) => ();
    ($name:ident, $($arg:tt)+) => ();
}

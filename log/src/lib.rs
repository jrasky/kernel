#![feature(macro_reexport)]
#![allow(improper_ctypes)]
#![feature(stmt_expr_attributes)]
#![feature(const_fn)]
#![feature(alloc)]
#![feature(collections)]
#![cfg_attr(not(test), no_std)]
#[cfg(not(test))]
extern crate core as std;
extern crate rlibc;
#[macro_use]
extern crate collections;
extern crate spin;
extern crate constants;
extern crate alloc;
#[macro_reexport(trace, debug, info, warn, error, critical)]
#[macro_use]
extern crate log_abi;

#[macro_use]
mod macros;
mod include;
mod logger;
mod point;

pub use log_abi::Location;
pub use logger::{Output, Request, has_output, set_output, set_reserve, reserve_log, log, set_level};
pub use point::{Frame, PointFrame, trace, write_trace};

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

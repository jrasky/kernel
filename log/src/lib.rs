#![feature(drop_types_in_const)]
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

pub use log_abi::{Location, level_name, to_level};
pub use logger::{Output, Request, has_output, set_output, set_reserve, reserve_log, log, set_level};
pub use point::{Frame, PointFrame, trace, write_trace};

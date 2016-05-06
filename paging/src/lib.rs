#![feature(stmt_expr_attributes)]
#![feature(shared)]
#![feature(unique)]
#![feature(collections_bound)]
#![feature(heap_api)]
#![feature(const_fn)]
#![feature(collections)]
#![feature(alloc)]
#![cfg_attr(not(test), no_std)]
#[cfg(not(test))]
extern crate core as std;
extern crate rlibc;
#[macro_use]
extern crate collections;
#[macro_use]
extern crate log;
extern crate alloc;
extern crate constants;
extern crate kernel_std;

pub use layout::Layout;
pub use segment::{Segment, raw_segment_size};
pub use table::{Entry, Table, Base};

mod include;
mod table;
mod segment;
mod layout;

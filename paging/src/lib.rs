#![feature(btree_range)]
#![feature(collections_bound)]
#![feature(heap_api)]
#![feature(ptr_as_ref)]
#![feature(set_recovery)]
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

pub use layout::Layout;
pub use segment::{Segment, raw_segment_size};
pub use allocator::{Allocator, Region};
pub use map::Map;

mod include;
mod segment;
mod frame;
mod layout;
mod page;
mod allocator;
mod map;

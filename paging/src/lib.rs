#![feature(btree_range)]
#![feature(collections_bound)]
#![feature(heap_api)]
#![feature(ptr_as_ref)]
#![feature(set_recovery)]
#![feature(const_fn)]
#![feature(collections)]
#![feature(alloc)]
#![cfg_attr(not(test), no_std)]
extern crate rlibc;
#[macro_use]
extern crate collections;
#[macro_use]
extern crate log;
extern crate alloc;

pub use layout::Layout;
pub use frame::Segment;
pub use allocator::{Allocator, Region};

pub use frame::raw_segment_size;

mod constants;
mod frame;
mod layout;
mod page;
mod allocator;

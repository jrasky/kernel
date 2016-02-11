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

pub use paging::Layout;
pub use paging::Segment;

mod paging;
mod constants;

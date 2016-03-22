pub use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
pub use std::fmt::{Debug, Formatter};

pub use collections::{Vec, BTreeMap, BTreeSet};
pub use collections::Bound::{Included, Unbounded, Excluded};

pub use alloc::raw_vec::RawVec;
pub use alloc::boxed::Box;

pub use alloc::heap;

pub use collections::btree_map;

pub use std::fmt;
pub use std::mem;
pub use std::cmp;
pub use std::ptr;

use constants::*;

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum FrameSize {
    Giant = 0x8000000000, // 512 gigabytes
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum PageSize {
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
    Page = 0x1000      // 4 kilobytes
}

pub unsafe fn get_pointer(place: *mut u64, idx: isize) -> Result<*mut u64, ()> {
    let entry = ptr::read(place.offset(idx));

    if entry & 1 == 0 || entry & (1 << 7) == 1 << 7 {
        // could not find table
        return Err(());
    }

    Ok(canonicalize((entry & PAGE_ADDR_MASK) as usize) as *mut _)
}

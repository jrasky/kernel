pub use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
pub use std::fmt::{Debug, Formatter};
pub use std::ptr::{Unique, Shared};

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

pub use constants::*;

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


impl PartialEq for FrameSize {
    fn eq(&self, other: &FrameSize) -> bool {
        *self as usize == *other as usize
    }
}

impl Eq for FrameSize {}

impl Ord for FrameSize {
    fn cmp(&self, other: &FrameSize) -> Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl PartialOrd for FrameSize {
    fn partial_cmp(&self, other: &FrameSize) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FrameSize {
    #[inline]
    pub fn get_shift(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }

    #[inline]
    pub fn get_pagesize(self) -> PageSize {
        match self {
            FrameSize::Giant => PageSize::Huge,
            FrameSize::Huge => PageSize::Big,
            FrameSize::Big => PageSize::Page
        }
    }

    #[inline]
    pub fn get_next(self) -> Option<FrameSize> {
        match self {
            FrameSize::Giant => Some(FrameSize::Huge),
            FrameSize::Huge => Some(FrameSize::Big),
            FrameSize::Big => None
        }
    }
}
impl PartialEq for PageSize {
    fn eq(&self, other: &PageSize) -> bool {
        *self as usize == *other as usize
    }
}

impl Eq for PageSize {}

impl Ord for PageSize {
    fn cmp(&self, other: &PageSize) -> Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl PartialOrd for PageSize {
    fn partial_cmp(&self, other: &PageSize) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PageSize {
    #[inline]
    pub fn get_shift(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }
}

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

pub use kernel_std::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSize {
    Giant, // 512 gigabytes
    Huge,  // 1 gigabyte
    Big,   // 2 megabytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    Huge, // 1 gigabyte
    Big,  // 2 megabytes
    Page  // 4 kilobytes
}

impl Ord for FrameSize {
    fn cmp(&self, other: &FrameSize) -> Ordering {
        self.get_size().cmp(&other.get_size())
    }
}

impl PartialOrd for FrameSize {
    fn partial_cmp(&self, other: &FrameSize) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Into<u64> for FrameSize {
    fn into(self) -> u64 {
        match self {
            FrameSize::Giant => 0x8000000000, // 512 gigabytes
            FrameSize::Huge  => 0x40000000,   // 1 gigabyte
            FrameSize::Big   => 0x200000,     // 2 megabytes
        }
    }
}

impl FrameSize {
    #[inline]
    pub fn get_size(self) -> u64 {
        self.into()
    }

    #[inline]
    pub fn get_shift(self) -> u64 {
        self.get_size().trailing_zeros() as u64
    }

    #[inline]
    pub fn get_pagesize(self) -> PageSize {
        match self {
            FrameSize::Giant => PageSize::Huge,
            FrameSize::Huge  => PageSize::Big,
            FrameSize::Big   => PageSize::Page
        }
    }

    #[inline]
    pub fn get_next(self) -> Option<FrameSize> {
        match self {
            FrameSize::Giant => Some(FrameSize::Huge),
            FrameSize::Huge  => Some(FrameSize::Big),
            FrameSize::Big => None
        }
    }
}

impl Ord for PageSize {
    fn cmp(&self, other: &PageSize) -> Ordering {
        self.get_size().cmp(&other.get_size())
    }
}

impl PartialOrd for PageSize {
    fn partial_cmp(&self, other: &PageSize) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Into<u64> for PageSize {
    fn into(self) -> u64 {
        match self {
            PageSize::Huge => 0x40000000, // 1 gigabyte
            PageSize::Big  => 0x200000,   // 2 megabytes
            PageSize::Page => 0x1000,     // 4 kilobytes
        }
    }
}

impl PageSize {
    #[inline]
    pub fn get_size(self) -> u64 {
        self.into()
    }

    #[inline]
    pub fn get_shift(self) -> u64 {
        self.get_size().trailing_zeros() as u64
    }
}

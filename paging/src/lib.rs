#![feature(stmt_expr_attributes)]
#![feature(shared)]
#![feature(const_fn)]
#![feature(collections)]
#![feature(alloc)]
#![feature(inclusive_range_syntax)]
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
pub use table::{Entry, Table, Base, Level, Info};
pub use builder::{Builder, build_layout_relative};

use std::cmp::{Ord, PartialOrd, Ordering};

mod table;
mod segment;
mod layout;
mod builder;

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

    #[inline]
    pub fn get_level(self) -> Level {
        match self {
            PageSize::Huge => Level::PDPTE,
            PageSize::Big => Level::PDE,
            PageSize::Page => Level::PTE
        }
    }

    #[inline]
    pub fn get_next(self) -> Option<PageSize> {
        match self {
            PageSize::Huge => Some(PageSize::Big),
            PageSize::Big => Some(PageSize::Page),
            PageSize::Page => None
        }
    }

    #[inline]
    pub fn get_framesize(self) -> FrameSize {
        match self {
            PageSize::Huge => FrameSize::Giant,
            PageSize::Big => FrameSize::Huge,
            PageSize::Page => FrameSize::Big
        }
    }
}

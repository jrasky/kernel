use super::PageSize;

use std::fmt::Debug;
use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};

use alloc::raw_vec::RawVec;
use alloc::boxed::Box;

use std::mem;
use std::fmt;

use constants::*;

/// Uniform linear address transformation
#[derive(Clone)]
pub struct Segment {
    physical_base: u64,
    virtual_base: u64,
    size: u64,
    allocate: bool,
    write: bool,
    user: bool,
    execute: bool,
    global: bool
}

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
struct RawSegment {
    physical_base: u64,
    virtual_base: u64,
    size: u64,
    flags: u8
}

pub fn raw_segment_size() -> usize {
    mem::size_of::<RawSegment>()
}

impl Debug for Segment {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Segment {{ physical_base: 0x{:x}, virtual_base: 0x{:x} size: 0x{:x}, allocate: {:?}, write: {:?}, user: {:?}, execute: {:?}, global: {:?} }}", self.physical_base, self.virtual_base, self.size, self.allocate, self.write, self.user, self.execute, self.global)
    }
}

// Overlap concerns virtual address only
// Segments can overlap on physical addresses and that's fine
impl Ord for Segment {
    fn cmp(&self, other: &Segment) -> Ordering {
        // aligned overlap check, since the page table is page-aligned
        if align(self.virtual_base + self.size, PageSize::Page as u64)
            <= align_back(other.virtual_base, PageSize::Page as u64) ||
            align_back(self.virtual_base, PageSize::Page as u64)
            >= align(other.virtual_base + other.size, PageSize::Page as u64) {
                self.physical_base.cmp(&other.virtual_base)
            } else {
                Ordering::Equal
            }
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Segment) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Segment {
    fn eq(&self, other: &Segment) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Segment {}


impl Segment {
    pub fn new(physical_base: u64, virtual_base: u64, size: u64,
               write: bool, user: bool, execute: bool, global: bool) -> Segment {
        debug_assert!(is_aligned(physical_base, 0x1000), "Physical base was not page-aligned");
        debug_assert!(is_aligned(virtual_base, 0x1000), "Virtual base was not aligned");

        Segment {
            physical_base: physical_base,
            virtual_base: virtual_base & ((1 << CANONICAL_BITS) - 1),
            allocate: true,
            size: size,
            write: write,
            user: user,
            execute: execute,
            global: global
        }
    }

    pub fn dummy_range(virtual_address: u64, size: u64) -> Segment {
        Segment {
            physical_base: 0,
            virtual_base: virtual_address & ((1 << CANONICAL_BITS) - 1),
            allocate: false,
            size: size,
            write: false,
            user: false,
            execute: false,
            global: false
        }
    }

    pub fn dummy(virtual_address: u64) -> Segment {
        Segment::dummy_range(virtual_address, 0)
    }

    pub fn physical_base(&self) -> u64 {
        self.physical_base
    }

    pub fn virtual_base(&self) -> u64 {
        self.virtual_base
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn write(&self) -> bool {
        self.write
    }

    pub fn user(&self) -> bool {
        self.user
    }

    pub fn execute(&self) -> bool {
        self.execute
    }

    pub fn global(&self) -> bool {
        self.global
    }

    pub fn from_raw(raw: &[u8]) -> Segment {
        assert!(raw.len() == mem::size_of::<RawSegment>());

        let data = unsafe {
            let ptr = raw.as_ptr() as *const RawSegment;
            ptr.as_ref().unwrap()
        };

        Segment {
            physical_base: data.physical_base,
            virtual_base: data.virtual_base,
            allocate: true,
            size: data.size,
            write: (data.flags & 1 << 0) == 1 << 0,
            user: (data.flags & 1 << 1) == 1 << 1,
            execute: (data.flags & 1 << 2) == 1 << 2,
            global: (data.flags & 1 << 3) == 1 << 3
        }
    }

    pub fn get_raw(&self) -> Box<[u8]> {
        let buffer: RawVec<u8> = RawVec::with_capacity(mem::size_of::<RawSegment>());
        let mut flags = 0;

        if self.write {
            flags |= 1 << 0;
        }

        if self.user {
            flags |= 1 << 1;
        }

        if self.execute {
            flags |= 1 << 2;
        }

        if self.global {
            flags |= 1 << 3;
        }

        let data = RawSegment {
            physical_base: self.physical_base,
            virtual_base: self.virtual_base,
            size: self.size,
            flags: flags
        };

        trace!("data: {:?}", data);

        unsafe {
            let ptr = buffer.ptr() as *mut RawSegment;
            *ptr.as_mut().unwrap() = data;

            buffer.into_box()
        }
    }

    #[inline]
    pub fn get_physical_subframe(&self, subframe_base: u64) -> u64 {
        if self.virtual_base > subframe_base {
            self.physical_base + self.virtual_base - subframe_base
        } else {
            self.physical_base + subframe_base - self.virtual_base
        }
    }

    #[inline]
    pub fn same_settings(&self, other: &Segment) -> bool {
        other.write == self.write &&
            other.user == self.user &&
            other.execute == self.execute &&
            other.global == self.global
    }
}

use collections::Vec;

#[cfg(not(test))]
use core::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
#[cfg(not(test))]
use core::fmt::{Debug, Formatter};

#[cfg(test)]
use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
#[cfg(test)]
use std::fmt::{Debug, Formatter};

use alloc::raw_vec::RawVec;
use alloc::boxed::Box;

use alloc::heap;

#[cfg(not(test))]
use core::fmt;
#[cfg(not(test))]
use core::mem;
#[cfg(not(test))]
use core::cmp;
#[cfg(not(test))]
use core::ptr;

#[cfg(test)]
use std::fmt;
#[cfg(test)]
use std::mem;
#[cfg(test)]
use std::cmp;
#[cfg(test)]
use std::ptr;

use constants::*;
use page::{Page, PageSize};

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
pub enum FrameSize {
    Giant = 0x8000000000, // 512 gigabytes
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
}

#[derive(Clone)]
pub struct Frame {
    size: FrameSize,
    base: usize, // virtual address
    entries: Vec<FrameEntry>
}

#[derive(Debug, Clone)]
pub enum FrameEntry {
    Empty,
    Page(Page),
    Frame(Frame)
}

/// Uniform linear address transformation
#[derive(Debug, Clone)]
pub struct Segment {
    physical_base: usize,
    virtual_base: usize,
    size: usize,
    allocate: bool,
    write: bool,
    user: bool,
    execute: bool,
    global: bool
}

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
struct RawSegment {
    physical_base: usize,
    virtual_base: usize,
    size: usize,
    flags: u8
}

pub fn raw_segment_size() -> usize {
    mem::size_of::<RawSegment>()
}

unsafe fn get_pointer(place: *mut u64, idx: isize) -> Result<*mut u64, ()> {
    let entry = ptr::read(place.offset(idx));

    if entry & 1 == 0 || entry & (1 << 7) == 1 << 7 {
        // could not find table
        return Err(());
    }

    Ok(canonicalize((entry & PAGE_ADDR_MASK) as usize) as *mut _)
}

// Overlap concerns virtual address only
// Segments can overlap on physical addresses and that's fine
impl Ord for Segment {
    fn cmp(&self, other: &Segment) -> Ordering {
        // aligned overlap check, since the page table is page-aligned
        if align(self.virtual_base + self.size, PageSize::Page as usize)
            <= align_back(other.virtual_base, PageSize::Page as usize) ||
            align_back(self.virtual_base, PageSize::Page as usize)
            >= align(other.virtual_base + other.size, PageSize::Page as usize) {
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


impl Debug for Frame {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        try!(write!(fmt, "Frame {{ size: {:?}, entries: [ ", self.size));
        let mut first = true;
        for entry in self.entries.iter() {
            if let &FrameEntry::Page(ref page) = entry {
                if first {
                    try!(write!(fmt, "{:?}", page));
                    first = false;
                } else {
                    try!(write!(fmt, " {:?}", page));
                }
            } else if let &FrameEntry::Frame(ref frame) = entry {
                if first {
                    try!(write!(fmt, "{:?}", frame));
                    first = false;
                } else {
                    try!(write!(fmt, " {:?}", frame));
                }
            }
        }
        try!(write!(fmt, "] }}"));

        // done
        Ok(())
    }
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

impl Segment {
    pub fn new(physical_base: usize, virtual_base: usize, size: usize,
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

    pub fn dummy(virtual_address: usize) -> Segment {
        Segment {
            physical_base: 0,
            virtual_base: virtual_address & ((1 << CANONICAL_BITS) - 1),
            allocate: false,
            size: 0,
            write: false,
            user: false,
            execute: false,
            global: false
        }
    }

    pub fn physical_base(&self) -> usize {
        self.physical_base
    }

    pub fn virtual_base(&self) -> usize {
        self.virtual_base
    }

    pub fn size(&self) -> usize {
        self.size
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
    fn get_physical_subframe(&self, subframe_base: usize) -> usize {
        if self.virtual_base > subframe_base {
            self.physical_base + self.virtual_base - subframe_base
        } else {
            self.physical_base + subframe_base - self.virtual_base
        }
    }

    unsafe fn build_edge(&self, place: *mut u64, base: usize, size: FrameSize,
                             idx: usize, vbase: usize) -> bool {
        trace!("0x{:x}, 0x{:x}", idx, vbase);

        if let Some(next) = size.get_next() {
            let subframe_base = base + (idx * size.get_pagesize() as usize);
            trace!("c 0x{:x}, 0x{:x}", subframe_base, vbase);
            if !is_aligned(self.get_physical_subframe(subframe_base), size.get_pagesize() as usize) ||
                (vbase >= subframe_base && vbase - subframe_base >= next.get_pagesize() as usize) ||
                subframe_base - vbase >= next.get_pagesize() as usize
            {
                // build a new table here
                if let Ok(ptr) = get_pointer(place, idx as isize) {
                    let result = self.build_into_inner(ptr, subframe_base, next);
                    debug_assert!(result, "Failed to insert subframe");
                    return true;
                } else {
                    return false;
                }
            }
        }

        false
    }

    unsafe fn build_page_at(&self, place: *mut u64, base: usize, size: FrameSize, idx: usize) {
        let subframe_base = base + (idx * size.get_pagesize() as usize);

        trace!("Creating page at 0x{:x} of size {:?}, idx 0x{:x}", subframe_base, size.get_pagesize(), idx);

        let page = Page {
            write: self.write,
            user: self.user,
            write_through: false,
            cache_disable: false,
            execute_disable: !self.execute,
            attribute_table: false,
            protection_key: 0,
            global: self.global,
            size: size.get_pagesize(),
            base: self.get_physical_subframe(subframe_base)
        };

        ptr::write(place.offset(idx as isize), page.get_entry());
    }

    unsafe fn build_into_inner(&self, place: *mut u64, base: usize, size: FrameSize) -> bool {
        trace!("0x{:x}, 0x{:x}, {:?}", place as usize, base, size);

        let min_idx = if base > self.virtual_base {
            0
        } else {
            align_back(self.virtual_base - base, size.get_pagesize() as usize) >> size.get_pagesize().get_shift()
        };

        let max_idx = cmp::min((align(self.virtual_base + self.size - base, size.get_pagesize() as usize)
                                >> size.get_pagesize().get_shift()), 0x200);

        trace!("0x{:x}, 0x{:x}", min_idx, max_idx);

        if max_idx == 0 || min_idx >= 0x200 {
            // cannot place segment here
            return false;
        }

        if !self.build_edge(place, base, size, min_idx, self.virtual_base) {
            self.build_page_at(place, base, size, min_idx);
        }

        trace!("a 0x{:x}, 0x{:x}, {:?}", place as usize, base, size);
        trace!("0x{:x}, 0x{:x}", min_idx, max_idx);

        if max_idx > min_idx + 1 {
            if !self.build_edge(place, base, size, max_idx - 1,
                                align_back(self.virtual_base + self.size as usize, size.get_pagesize() as usize)) {
                self.build_page_at(place, base, size, max_idx - 1);
            }
        }

        trace!("b 0x{:x}, 0x{:x}, {:?}", place as usize, base, size);

        for idx in min_idx + 1..max_idx - 1 {
            if !self.build_edge(place, base, size, idx, base + (idx * size.get_pagesize() as usize)) {
                self.build_page_at(place, base, size, idx)
            }
        }

        true
    }

    #[inline]
    pub unsafe fn build_into(&self, place: *mut u64) -> bool {
        if !self.allocate {
            return false;
        }

        let min_idx = align_back(self.virtual_base, FrameSize::Giant as usize)
            >> FrameSize::Giant.get_shift();
        let max_idx = align(self.virtual_base + self.size, FrameSize::Giant as usize)
            >> FrameSize::Giant.get_shift();

        for idx in min_idx..max_idx {
            if let Ok(ptr) = get_pointer(place, idx as isize) {
                if !self.build_into_inner(
                    ptr, idx * FrameSize::Giant as usize, FrameSize::Giant)
                {
                    return false;
                }
            } else {
                // failed to find table
                return false;
            }
        }

        true
    }

    #[inline]
    pub fn get_indicies(&self, size: FrameSize) -> (usize, usize) {
        (align_back(self.virtual_base, size as usize) >> size.get_shift(),
         align(self.virtual_base + self.size, size as usize) >> size.get_shift())
    }

    #[inline]
    pub fn same_settings(&self, other: &Segment) -> bool {
        other.write == self.write &&
            other.user == self.user &&
            other.execute == self.execute &&
            other.global == self.global
    }
}

impl Frame {
    pub fn new(size: FrameSize, base: usize) -> Frame {
        debug_assert!(is_aligned(base, size.get_pagesize() as usize),
                      "Frame base was not aligned to page size");

        let mut entries = Vec::with_capacity(0x200);

        for _ in 0..0x200 {
            entries.push(FrameEntry::Empty);
        }

        Frame {
            size: size,
            base: base,
            entries: entries
        }
    }

    #[cfg(test)]
    #[inline]
    pub fn get_entry(&self, idx: usize) -> &FrameEntry {
        &self.entries[idx]
    }

    pub unsafe fn build_table_into(&self, buffers: &mut Vec<RawVec<u64>>, place: *mut u64, additional: bool) {
        for idx in 0..0x200 {
            match self.entries[idx] {
                FrameEntry::Empty => {
                    // clear the entry
                    if !additional {
                        *place.offset(idx as isize).as_mut().unwrap() = 0;
                    }
                },
                FrameEntry::Page(ref page) => {
                    // overwrite the page
                    *place.offset(idx as isize).as_mut().unwrap() =
                        page.get_entry();
                },
                FrameEntry::Frame(ref frame) => {
                    if *place.offset(idx as isize).as_ref().unwrap() == 0 {
                        // create a new frame buffer
                        *place.offset(idx as isize).as_mut().unwrap() =
                            frame.build_table(buffers);
                    } else {
                        // write into the existing one
                        let entry = *place.offset(idx as isize).as_ref().unwrap();
                        frame.build_table_into(buffers, (entry & PAGE_ADDR_MASK) as *mut _, additional);
                    }
                }
            }
        }
    }

    pub fn build_table_relative(&self, base: usize, buffer: &mut Vec<u64>) -> u64 {
        // create out own temporary buffer
        let mut inner_buffer: Vec<u64> = vec![];

        for idx in 0..0x200 {
            match self.entries[idx] {
                FrameEntry::Empty => {
                    inner_buffer.push(0);
                },
                FrameEntry::Page(ref page) => {
                    inner_buffer.push(page.get_entry());
                },
                FrameEntry::Frame(ref frame) => {
                    inner_buffer.push(frame.build_table_relative(base, buffer));
                }
            }
        }

        // align our position in our buffer

        for _ in buffer.len()..align(buffer.len(), 0x200) {
            buffer.push(0);
        }

        // get our position
        let entry = (buffer.len() * U64_BYTES + base) as u64 | 0x7;

        // copy our buffer
        buffer.extend(inner_buffer);

        trace!("Frame entry: 0x{:x}", entry);

        // produce our entry
        entry
    }

    pub fn build_table(&self, buffers: &mut Vec<RawVec<u64>>) -> u64 {
        let buffer: RawVec<u64> = unsafe {
            // our buffer needs to be page-aligned
            RawVec::from_raw_parts(heap::allocate(mem::size_of::<u64>() * 0x200, 0x1000) as *mut _, 0x200)
        };

        for idx in 0..0x200 {
            match self.entries[idx] {
                FrameEntry::Empty => {
                    unsafe {*buffer.ptr().offset(idx as isize).as_mut().unwrap() = 0};
                },
                FrameEntry::Page(ref page) => {
                    unsafe {*buffer.ptr().offset(idx as isize).as_mut().unwrap() = page.get_entry()};
                },
                FrameEntry::Frame(ref frame) => {
                    unsafe {*buffer.ptr().offset(idx as isize).as_mut().unwrap() =
                        frame.build_table(buffers)};
                }
            }
        }

        let entry = buffer.ptr() as u64 | 0x7;

        buffers.push(buffer);

        trace!("Frame entry: 0x{:x}", entry);

        entry
    }

    pub fn merge(&mut self, segment: Segment, remove: bool) {
        // maximum index into our entries that we should write
        let max_idx = cmp::min(align(segment.virtual_base + segment.size - self.base,
                                     self.size.get_pagesize() as usize)
                               >> self.size.get_pagesize().get_shift(), 0x200 - 1);

        trace!("{:x}, {:x}", align(segment.virtual_base + segment.size - self.base,
                                     self.size.get_pagesize() as usize),
               self.size.get_pagesize().get_shift());

        let min_idx;

        trace!("{:x}, {:x}, {:x}, {:?}", segment.virtual_base, segment.size, self.base, self.size);

        // figure out the starting index
        if align_back(segment.virtual_base, self.size.get_pagesize() as usize) >=
            self.base && segment.virtual_base < self.base + self.size as usize {
            // segment begins past or at our base, but before our end
            min_idx = align_back(segment.virtual_base - self.base, self.size.get_pagesize() as usize)
                    >> self.size.get_pagesize().get_shift();
        } else if align(segment.virtual_base + segment.size, self.size.get_pagesize() as usize) > self.base {
            // segment starts before our base
            min_idx = 0;
        } else {
            unreachable!("Merge called with non-overlapping section");
        }

        trace!("{:x}, {:x}", min_idx, max_idx);

        // merge into those indexes
        for idx in min_idx..max_idx {
            self.merge_at_index(segment.clone(), idx, remove);
        }
    }

    fn merge_at_index(&mut self, segment: Segment, idx: usize, remove: bool) {
        // need a separate branch factor
        let branch: usize;

        match self.entries[idx] {
            FrameEntry::Empty => {
                branch = 0;
            },
            FrameEntry::Page(_) => {
                branch = 1;
            },
            FrameEntry::Frame(ref mut frame) => {
                frame.merge(segment, remove);
                return;
            }
        }

        match branch {
            0 => {
                if !remove {
                    self.merge_new(segment, idx)
                }
            },
            1 => {
                if remove {
                    // clear anything overlapping, so don't split the section
                    self.entries[idx] = FrameEntry::Empty;
                } else {
                    unreachable!("Merge called with an overlapping section")
                }
            },
            _ => {
                unreachable!();
            }
        }
    }

    fn merge_new(&mut self, segment: Segment, idx: usize) {
        trace!("Creating new page");
        let subframe_base = self.base + (idx * (self.size.get_pagesize() as usize));
        let physical_base;

        trace!("{:x}, {:x}, {:x}, {:?}, {:x}", segment.virtual_base, segment.size, subframe_base, self.size, self.size.get_pagesize() as usize);

        if segment.virtual_base + segment.size < subframe_base + self.size.get_pagesize() as usize {
            if self.size.get_pagesize() as usize + subframe_base - segment.virtual_base - segment.size
                >= PageSize::Page as usize {
                    if let Some(next_size) = self.size.get_next() {
                        // subframe should be split further
                        let mut new_frame = Frame::new(next_size, subframe_base);
                        // not removing if we're in this function
                        new_frame.merge(segment, false);
                        self.entries[idx] = FrameEntry::Frame(new_frame);
                    
                        // done
                        return;
                    }
                }
        }

        if segment.virtual_base > subframe_base {
            if segment.virtual_base - subframe_base >= PageSize::Page as usize {
                if let Some(next_size) = self.size.get_next() {
                    // subframe should be split further
                    let mut new_frame = Frame::new(next_size, subframe_base);
                    // not removing if we're in this function
                    new_frame.merge(segment, false);
                    self.entries[idx] = FrameEntry::Frame(new_frame);
                
                    // done
                    return;
                }
            }
            physical_base = segment.physical_base + segment.virtual_base - subframe_base;
        } else {
            physical_base = segment.physical_base + subframe_base - segment.virtual_base;
        }

        // just create a page here
        self.entries[idx] = FrameEntry::Page(Page {
            write: segment.write,
            user: segment.user,
            write_through: false,
            cache_disable: false,
            execute_disable: !segment.execute,
            attribute_table: false,
            protection_key: 0,
            global: segment.global,
            size: self.size.get_pagesize(),
            base: physical_base
        });
    }
}


use collections::{BTreeSet, Vec};

#[cfg(not(test))]
use core::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
#[cfg(not(test))]
use core::fmt::{Debug, Formatter};

#[cfg(test)]
use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
#[cfg(test)]
use std::fmt::{Debug, Formatter};

use alloc::raw_vec::RawVec;

use alloc::heap;

#[cfg(not(test))]
use core::fmt;
#[cfg(not(test))]
use core::mem;
#[cfg(not(test))]
use core::cmp;

#[cfg(test)]
use std::fmt;
#[cfg(test)]
use std::mem;
#[cfg(test)]
use std::cmp;

use constants::*;

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
enum PageSize {
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
    Page = 0x1000      // 4 kilobytes
}

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
enum FrameSize {
    Giant = 0x8000000000, // 512 gigabytes
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
}

/// Uniform linear address transformation
#[derive(Debug, Clone)]
pub struct Segment {
    physical_base: usize,
    virtual_base: usize,
    size: usize,
    write: bool,
    user: bool,
    execute: bool,
    global: bool
}

pub struct Layout {
    entries: Vec<Option<Frame>>,
    map: BTreeSet<Segment>, // use a map for convenience
    buffers: Vec<RawVec<u64>>
}

#[derive(Clone)]
struct Frame {
    size: FrameSize,
    base: usize, // virtual address
    entries: Vec<FrameEntry>
}

#[derive(Debug, Clone)]
enum FrameEntry {
    Empty,
    Page(Page),
    Frame(Frame)
}

/// Access settings for physical memory
#[derive(Debug, Clone, PartialEq)]
struct Page {
    write: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    execute_disable: bool,
    attribute_table: bool,
    protection_key: u8,
    global: bool,
    size: PageSize,
    base: usize
}

impl Debug for Layout {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        try!(write!(fmt, "Layout [ "));
        let mut first = true;
        for entry in self.entries.iter() {
            if let &Some(ref entry) = entry {
                if first {
                    try!(write!(fmt, "{:?}", entry));
                    first = false;
                } else {
                    try!(write!(fmt, " {:?}", entry));
                }
            }
        }
        try!(write!(fmt, "]"));

        // done
        Ok(())
    }
}

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

impl Eq for Page {}

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

impl Page {
    fn get_entry(&self) -> u64 {
        let mut entry = (self.protection_key as u64) << 59 | self.base as u64 | (1 << 0);

        if self.execute_disable {
            entry |= 1 << 63;
        }

        if self.global {
            entry |= 1 << 8;
        }

        if self.size != PageSize::Page || self.attribute_table {
            entry |= 1 << 7;
        } else if self.attribute_table {
            entry |= 1 << 12;
        }

        if self.cache_disable {
            entry |= 1 << 4;
        }

        if self.write_through {
            entry |= 1 << 3;
        }

        if self.user {
            entry |= 1 << 2;
        }

        if self.write {
            entry |= 1 << 1;
        }

        trace!("Page entry: {:x}", entry);

        entry
    }
}

impl Segment {
    pub fn new(physical_base: usize, virtual_base: usize, size: usize,
               write: bool, user: bool, execute: bool, global: bool) -> Segment {
        Segment {
            physical_base: physical_base,
            virtual_base: virtual_base & ((1 << CANONICAL_BITS) - 1),
            size: size,
            write: write,
            user: user,
            execute: execute,
            global: global
        }
    }
}

impl PageSize {
    #[inline]
    fn get_shift(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }
}

impl FrameSize {
    #[inline]
    fn get_shift(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }

    #[inline]
    fn get_pagesize(self) -> PageSize {
        match self {
            FrameSize::Giant => PageSize::Huge,
            FrameSize::Huge => PageSize::Big,
            FrameSize::Big => PageSize::Page
        }
    }

    #[inline]
    fn get_next(self) -> Option<FrameSize> {
        match self {
            FrameSize::Giant => Some(FrameSize::Huge),
            FrameSize::Huge => Some(FrameSize::Big),
            FrameSize::Big => None
        }
    }
}

impl Layout {
    pub fn new() -> Layout {
        let mut entries = Vec::with_capacity(0x200);

        for _ in 0..0x200 {
            entries.push(None);
        }

        Layout {
            entries: entries,
            map: BTreeSet::new(),
            buffers: vec![]
        }
    }

    pub fn build_tables(&mut self) -> u64 {
        let buffer: RawVec<u64> = unsafe {
            // our buffer needs to be page-aligned
            RawVec::from_raw_parts(heap::allocate(mem::size_of::<u64>() * 0x200, 0x1000) as *mut _, 0x200)
        };

        for idx in 0..0x200 {
            match self.entries[idx] {
                None => {
                    unsafe {*buffer.ptr().offset(idx as isize).as_mut().unwrap() = 0};
                },
                Some(ref frame) => {
                    unsafe {*buffer.ptr().offset(idx as isize).as_mut().unwrap() =
                        frame.build_table(&mut self.buffers)};
                }
            }
        }

        let entry = buffer.ptr() as u64;

        self.buffers.push(buffer);

        trace!("Layout entry: 0x{:x}", entry);

        entry
    }

    pub fn build_tables_relative(&self, base: usize) -> Vec<u8> {
        vec![]
    }

    pub fn insert(&mut self, segment: Segment) -> bool {
        if self.map.insert(segment.clone()) {
            trace!("Inserting segment {:?}", &segment);
            self.merge(segment, false);
            true
        } else {
            if let Some(old_segment) = self.map.get(&segment) {
                if old_segment.write != segment.write ||
                    old_segment.user != segment.user ||
                    old_segment.execute != segment.execute ||
                    old_segment.global != segment.global {
                        warn!("Failed to insert overlapping segment: {:?}, overlapped by {:?}",
                              segment, old_segment);
                    }
            }
            false
        }
    }

    #[allow(dead_code)] // will use eventually
    pub fn remove(&mut self, segment: Segment) -> bool {
        if self.map.remove(&segment) {
            self.merge(segment, true);
            true
        } else {
            false
        }
    }

    fn merge(&mut self, segment: Segment, remove: bool) {
        let min_idx = align_back(segment.virtual_base, FrameSize::Giant as usize) >> FrameSize::Giant.get_shift();

        let max_idx = align(segment.virtual_base + segment.size, FrameSize::Giant as usize) >> FrameSize::Giant.get_shift();

        trace!("{:?}, {:?}", align(segment.virtual_base + segment.size, FrameSize::Giant as usize), FrameSize::Giant.get_shift());
        trace!("{:?}, {:?}", min_idx, max_idx);

        for idx in min_idx..max_idx {
            let mut new_frame = None;
            if let Some(ref mut entry) = self.entries[idx] {
                trace!("Merging with existing section");
                entry.merge(segment.clone(), remove);
            } else {
                trace!("Creating new section");
                let mut frame = Frame::new(FrameSize::Giant, idx << FrameSize::Giant.get_shift());
                frame.merge(segment.clone(), remove);
                new_frame = Some(frame);
            }

            if new_frame.is_some() {
                self.entries[idx] = new_frame;
            }
        }

        trace!("{:?}", self);
    }
}

impl Frame {
    fn new(size: FrameSize, base: usize) -> Frame {
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

    fn build_table_relative(&self, base: usize, buffer: &mut Vec<u64>) -> u64 {
        
    }

    fn build_table(&self, buffers: &mut Vec<RawVec<u64>>) -> u64 {
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

    fn merge(&mut self, segment: Segment, remove: bool) {
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

#[cfg(test)]
#[test]
fn test_layout() {
    let mut layout = Layout::new();
    layout.insert(Segment::new(0x200000, 0x200000, 0x4000, false, false, false, false));
    if let Some(ref giant_frame) = layout.entries[0] { // giant
        if let FrameEntry::Frame(ref huge_frame) = giant_frame.entries[0] { // huge
            if let FrameEntry::Frame(ref big_frame) = huge_frame.entries[1] { // big
                if let FrameEntry::Page(ref page) = big_frame.entries[0] { // page
                    assert!(page.write == false, "Page was write-enabled");
                    assert!(page.user == false, "Page was user-mod");
                    assert!(page.execute_disable == true, "Page was executable");
                    assert!(page.global == false, "Page was global");
                    assert!(page.size == PageSize::Page, "Page was the wrong size");
                    assert!(page.base == 0x200000, "Page was not in the right place");
                } else {
                    panic!("Did not find page inside big frame: {:?}", big_frame.entries[0]);
                }
            } else {
                panic!("Did not find frame inside huge frame: {:?}", huge_frame.entries[1]);
            }
        } else {
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.entries[0]);
        }
    } else {
        panic!("Did not find giant frame");
    }
}

#[cfg(test)]
#[test]
fn test_layout_high() {
    let mut layout = Layout::new();
    layout.insert(Segment::new(0x200000, 0xffffffff80200000, 0x4000, false, false, false, false));
    if let Some(ref giant_frame) = layout.entries[511] { // giant
        if let FrameEntry::Frame(ref huge_frame) = giant_frame.entries[510] { // huge
            if let FrameEntry::Frame(ref big_frame) = huge_frame.entries[1] { // big
                if let FrameEntry::Page(ref page) = big_frame.entries[0] { // page
                    assert!(page.write == false, "Page was write-enabled");
                    assert!(page.user == false, "Page was user-mod");
                    assert!(page.execute_disable == true, "Page was executable");
                    assert!(page.global == false, "Page was global");
                    assert!(page.size == PageSize::Page, "Page was the wrong size");
                    assert!(page.base == 0x200000, "Page was not in the right place");
                } else {
                    panic!("Did not find page inside big frame: {:?}", big_frame.entries[0]);
                }
            } else {
                panic!("Did not find frame inside huge frame: {:?}", huge_frame.entries[1]);
            }
        } else {
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.entries[510]);
        }
    } else {
        panic!("Did not find giant frame");
    }
}

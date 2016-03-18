use collections::{BTreeSet, Vec};

#[cfg(not(test))]
use core::fmt::{Debug, Formatter};

#[cfg(test)]
use std::fmt::{Debug, Formatter};

use alloc::raw_vec::RawVec;

use alloc::heap;

#[cfg(not(test))]
use core::fmt;
#[cfg(not(test))]
use core::mem;

#[cfg(test)]
use std::fmt;
#[cfg(test)]
use std::mem;

use constants::*;
use frame::{Frame, Segment, FrameSize};
use allocator::{Allocator, Region};

pub struct Layout {
    entries: Vec<Option<Frame>>,
    map: BTreeSet<Segment>,
    free: Allocator,
    buffers: Vec<RawVec<u64>>
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

impl Layout {
    pub fn new() -> Layout {
        let mut entries = Vec::with_capacity(0x200);

        for _ in 0..0x200 {
            entries.push(None);
        }

        Layout {
            entries: entries,
            map: BTreeSet::new(),
            free: Allocator::new(),
            buffers: vec![]
        }
    }

    #[inline]
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        self.free.allocate(size, align)
    }

    #[inline]
    pub fn register(&mut self, region: Region) -> bool {
        self.free.register(region)
    }

    #[inline]
    pub fn forget(&mut self, region: Region) -> bool {
        self.free.forget(region)
    }
    
    pub fn insert(&mut self, segment: Segment) -> bool {
        if self.map.insert(segment.clone()) {
            let region = Region::new(segment.virtual_base(), segment.size());
            // may or may not already be allocated
            self.free.set_used(region);

            trace!("Inserting segment {:?}", &segment);
            self.merge(segment, false);
            true
        } else {
            if let Some(old_segment) = self.map.get(&segment) {
                if !old_segment.same_settings(&segment) {
                        warn!("Failed to insert overlapping segment: {:?}, overlapped by {:?}",
                              segment, old_segment);
                    }
            }
            false
        }
    }

    pub fn remove(&mut self, segment: Segment) -> bool {
        let region = Region::new(segment.virtual_base(), segment.size());

        if self.free.release(region) && self.map.remove(&segment) {
            self.merge(segment, true);
            true
        } else {
            false
        }
    }

    pub unsafe fn build_tables_into(&mut self, tables: *mut u64, additional: bool) {
        for idx in 0..0x200 {
            if let Some(ref frame) = self.entries[idx] {
                if *tables.offset(idx as isize).as_ref().unwrap() == 0 {
                    *tables.offset(idx as isize).as_mut().unwrap() =
                        frame.build_table(&mut self.buffers);
                } else {
                    let entry = *tables.offset(idx as isize).as_ref().unwrap();
                    frame.build_table_into(&mut self.buffers, (entry & PAGE_ADDR_MASK) as *mut _, additional);
                }
            } else if !additional {
                *tables.offset(idx as isize).as_mut().unwrap() = 0;
            }
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

    pub fn build_tables_relative(&self, base: usize) -> (u64, Vec<u64>) {
        let mut buffer = vec![];

        let mut inner_buffer: Vec<u64> = vec![];

        for idx in 0..0x200 {
            match self.entries[idx] {
                None => {
                    inner_buffer.push(0);
                },
                Some(ref frame) => {
                    inner_buffer.push(frame.build_table_relative(base, &mut buffer));
                }
            }
        }

        trace!("Inner buffer length: {}", buffer.len());

        // align our position in our buffer
        for _ in buffer.len()..align(buffer.len(), 0x200) {
            buffer.push(0);
        }

        trace!("Inner buffer length after alignment: {}", buffer.len());

        // get our position
        let entry = (buffer.len() * U64_BYTES + base) as u64;

        // copy our buffer in
        buffer.extend(inner_buffer);

        // produce our position and the constructed buffer
        (entry, buffer)
    }

    pub fn to_physical(&self, addr: usize) -> Option<usize> {
        // TODO: Implement this
        None
    }

    fn merge(&mut self, segment: Segment, remove: bool) {
        let (min_idx, max_idx) = segment.get_indicies(FrameSize::Giant);

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

#[cfg(test)]
#[test]
fn test_layout() {
    use frame::FrameEntry;
    use page::PageSize;
    let mut layout = Layout::new();
    layout.insert(Segment::new(0x200000, 0x200000, 0x4000, false, false, false, false));
    if let Some(ref giant_frame) = layout.entries[0] { // giant
        if let &FrameEntry::Frame(ref huge_frame) = giant_frame.get_entry(0) { // huge
            if let &FrameEntry::Frame(ref big_frame) = huge_frame.get_entry(1) { // big
                if let &FrameEntry::Page(ref page) = big_frame.get_entry(0) { // page
                    assert!(page.write == false, "Page was write-enabled");
                    assert!(page.user == false, "Page was user-mode");
                    assert!(page.execute_disable == true, "Page was executable");
                    assert!(page.global == false, "Page was global");
                    assert!(page.size == PageSize::Page, "Page was the wrong size");
                    assert!(page.base == 0x200000, "Page was not in the right place");
                } else {
                    panic!("Did not find page inside big frame: {:?}", big_frame.get_entry(0));
                }
            } else {
                panic!("Did not find frame inside huge frame: {:?}", huge_frame.get_entry(1));
            }
        } else {
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.get_entry(0));
        }
    } else {
        panic!("Did not find giant frame");
    }
}

#[cfg(test)]
#[test]
fn test_layout_high() {
    use frame::FrameEntry;
    use page::PageSize;
    let mut layout = Layout::new();
    layout.insert(Segment::new(0x200000, 0xffffffff80200000, 0x4000, false, false, true, false));
    if let Some(ref giant_frame) = layout.entries[511] { // giant
        if let &FrameEntry::Frame(ref huge_frame) = giant_frame.get_entry(510) { // huge
            if let &FrameEntry::Frame(ref big_frame) = huge_frame.get_entry(1) { // big
                if let &FrameEntry::Page(ref page) = big_frame.get_entry(0) { // page
                    assert!(page.write == false, "Page was write-enabled");
                    assert!(page.user == false, "Page was user-mode");
                    assert!(page.execute_disable == false, "Page was not executable");
                    assert!(page.global == false, "Page was global");
                    assert!(page.size == PageSize::Page, "Page was the wrong size");
                    assert!(page.base == 0x200000, "Page was not in the right place");
                } else {
                    panic!("Did not find page inside big frame: {:?}", big_frame.get_entry(0));
                }
            } else {
                panic!("Did not find frame inside huge frame: {:?}", huge_frame.get_entry(1));
            }
        } else {
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.get_entry(510));
        }
    } else {
        panic!("Did not find giant frame");
    }
}

#[cfg(test)]
#[test]
fn test_layout_data() {
    use frame::FrameEntry;
    use page::PageSize;
    let mut layout = Layout::new();
    layout.insert(Segment::new(0, 0, 0x200000,
                               false, false, true, false));
    layout.insert(Segment::new(0x200000, 0xffff80200000, 0x34ddf,
                               false, false, true, false));
    layout.insert(Segment::new(0x235000, 0xffff80400000, 0x4fac,
                               false, false, false, false));
    layout.insert(Segment::new(0x23b000, 0xffff80600000, 0x4a00,
                               true, false, false, false));
    layout.insert(Segment::new(0x23f000, 0xffff80800000, 0x17648,
                               true, false, false, false));

    if let Some(ref giant_frame) = layout.entries[511] { // giant
        if let &FrameEntry::Frame(ref huge_frame) = giant_frame.get_entry(510) { // huge
            if let &FrameEntry::Frame(ref big_frame) = huge_frame.get_entry(3) { // big
                if let &FrameEntry::Page(ref page) = big_frame.get_entry(4) { // page
                    assert!(page.write == true, "Page was read-only");
                    assert!(page.user == false, "Page was user-mode");
                    assert!(page.execute_disable == true, "Page was executable");
                    assert!(page.global == false, "Page was global");
                    assert!(page.size == PageSize::Page, "Page was the wrong size");
                    assert!(page.base == 0x23f000, "Page was not in the right place");
                } else {
                    panic!("Did not find page inside big frame: {:?}", big_frame.get_entry(4));
                }
            } else {
                panic!("Did not find frame inside huge frame: {:?}", huge_frame.get_entry(3));
            }
        } else {
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.get_entry(510));
        }
    } else {
        panic!("Did not find giant frame");
    }
}

#[cfg(test)]
#[test]
fn test_walk_data() {
    let mut layout = Layout::new();

    layout.insert(Segment::new(0, 0, 0x200000,
                               false, false, true, false));
    layout.insert(Segment::new(0x200000, 0xffff80200000, 0x34ddf,
                               false, false, true, false));
    layout.insert(Segment::new(0x235000, 0xffff80400000, 0x4fac,
                               false, false, false, false));
    layout.insert(Segment::new(0x23b000, 0xffff80600000, 0x4a00,
                               true, false, false, false));
    layout.insert(Segment::new(0x23f000, 0xffff80800000, 0x17648,
                               true, false, false, false));

    let (addr, tables) = layout.build_tables_relative(0x180000);
    let head_idx = (addr - 0x180000) / 0x8;

    // 511th entry
    let next_entry = tables[head_idx as usize + 511];
    assert!(next_entry & 0x1 == 0x1, "No 511th entry");
    let next_addr = next_entry & (((1 << 41) - 1) << 12);
    let next_idx = (next_addr - 0x180000) / 0x8;
    
    // 510th entry
    let next_entry = tables[next_idx as usize + 510];
    assert!(next_entry & 0x1 == 0x1, "No 510th entry");
    assert!(next_entry & 0x80 == 0, "510th entry was a frame");
    let next_addr = next_entry & (((1 << 41) - 1) << 12);
    let next_idx = (next_addr - 0x180000) / 0x8;

    // 3rd entry
    let next_entry = tables[next_idx as usize + 3];
    assert!(next_entry & 0x1 == 0x1, "No 3rd entry");
    assert!(next_entry & 0x80 == 0, "3rd entry was a frame");
    let next_addr = next_entry & (((1 << 41) - 1) << 12);
    let next_idx = (next_addr - 0x180000) / 0x8;

    // 4th
    let next_entry = tables[next_idx as usize + 4];
    assert!(next_entry & 0x1 == 0x1, "No 4th entry");
    let frame_addr = next_entry & (((1 << 41) - 1) << 12);
    assert!(frame_addr == 0x23f000, "Frame was not at the right address: 0x{:x}", frame_addr);
}

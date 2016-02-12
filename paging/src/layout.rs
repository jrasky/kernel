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

pub struct Layout {
    entries: Vec<Option<Frame>>,
    map: BTreeSet<Segment>, // use a map for convenience
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

    pub fn insert(&mut self, segment: Segment) -> bool {
        if self.map.insert(segment.clone()) {
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
                    assert!(page.user == false, "Page was user-mod");
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
    layout.insert(Segment::new(0x200000, 0xffffffff80200000, 0x4000, false, false, false, false));
    if let Some(ref giant_frame) = layout.entries[511] { // giant
        if let &FrameEntry::Frame(ref huge_frame) = giant_frame.get_entry(510) { // huge
            if let &FrameEntry::Frame(ref big_frame) = huge_frame.get_entry(1) { // big
                if let &FrameEntry::Page(ref page) = big_frame.get_entry(0) { // page
                    assert!(page.write == false, "Page was write-enabled");
                    assert!(page.user == false, "Page was user-mod");
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
            panic!("Did not find frame inside giant frame: {:?}", giant_frame.get_entry(512));
        }
    } else {
        panic!("Did not find giant frame");
    }
}
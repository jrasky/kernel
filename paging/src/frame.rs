use include::*;

use page::Page;
use segment::Segment;

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
                            frame.build_table(buffers, &|x| x);
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

    pub unsafe fn build_table<F>(&self, buffers: &mut Vec<RawVec<u64>>,
                          translate: &F) -> u64 where F: Fn(u64) -> u64 {
        let buffer: RawVec<u64> = RawVec::from_raw_parts(heap::allocate(mem::size_of::<u64>() * 0x200, 0x1000) as *mut _, 0x200);

        for idx in 0..0x200 {
            match self.entries[idx] {
                FrameEntry::Empty => {
                    *buffer.ptr().offset(idx as isize).as_mut().unwrap() = 0;
                },
                FrameEntry::Page(ref page) => {
                    *buffer.ptr().offset(idx as isize).as_mut().unwrap() = page.get_entry();
                },
                FrameEntry::Frame(ref frame) => {
                    *buffer.ptr().offset(idx as isize).as_mut().unwrap() =
                        frame.build_table(buffers, translate);
                }
            }
        }

        let entry = translate(buffer.ptr() as u64) | 0x7;

        buffers.push(buffer);

        trace!("Frame entry: 0x{:x}", entry);

        entry
    }

    pub fn merge(&mut self, segment: Segment, remove: bool) {
        // maximum index into our entries that we should write
        let max_idx = cmp::min(align(segment.virtual_base() + segment.size() - self.base,
                                     self.size.get_pagesize() as usize)
                               >> self.size.get_pagesize().get_shift(), 0x200 - 1);

        trace!("{:x}, {:x}", align(segment.virtual_base() + segment.size() - self.base,
                                     self.size.get_pagesize() as usize),
               self.size.get_pagesize().get_shift());

        let min_idx;

        trace!("{:x}, {:x}, {:x}, {:?}", segment.virtual_base(), segment.size(), self.base, self.size);

        // figure out the starting index
        if align_back(segment.virtual_base(), self.size.get_pagesize() as usize) >=
            self.base && segment.virtual_base() < self.base + self.size as usize {
            // segment begins past or at our base, but before our end
            min_idx = align_back(segment.virtual_base() - self.base, self.size.get_pagesize() as usize)
                    >> self.size.get_pagesize().get_shift();
        } else if align(segment.virtual_base() + segment.size(), self.size.get_pagesize() as usize) > self.base {
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

        trace!("{:x}, {:x}, {:x}, {:?}, {:x}", segment.virtual_base(), segment.size(), subframe_base, self.size, self.size.get_pagesize() as usize);

        if segment.virtual_base() + segment.size() < subframe_base + self.size.get_pagesize() as usize {
            if self.size.get_pagesize() as usize + subframe_base - segment.virtual_base() - segment.size()
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

        if segment.virtual_base() > subframe_base {
            if segment.virtual_base() - subframe_base >= PageSize::Page as usize {
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
            physical_base = segment.physical_base() + segment.virtual_base() - subframe_base;
        } else {
            physical_base = segment.physical_base() + subframe_base - segment.virtual_base();
        }

        // just create a page here
        self.entries[idx] = FrameEntry::Page(Page {
            write: segment.write(),
            user: segment.user(),
            write_through: false,
            cache_disable: false,
            execute_disable: !segment.execute(),
            attribute_table: false,
            protection_key: 0,
            global: segment.global(),
            size: self.size.get_pagesize(),
            base: physical_base
        });
    }
}


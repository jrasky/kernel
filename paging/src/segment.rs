use include::*;

use page::Page;

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

    pub fn dummy_range(virtual_address: usize, size: usize) -> Segment {
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

    pub fn dummy(virtual_address: usize) -> Segment {
        Segment::dummy_range(virtual_address, 0)
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
    pub fn get_physical_subframe(&self, subframe_base: usize) -> usize {
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

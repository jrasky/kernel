use include::*;

use segment::Segment;
use allocator::{Allocator, Region};
use table::{Info, Table, Base, Level};

pub struct Layout {
    map: BTreeSet<Segment>,
    free: Allocator,
}

struct RelativeBuilder {
    base: usize,
    used: usize,
    tables: Vec<(Box<Table>, usize)>,
    to_physical: BTreeMap<usize, usize>,
    to_virtual: BTreeMap<usize, usize>
}

impl Base for RelativeBuilder {
    fn to_virtual(&self, address: usize) -> Option<usize> {
        self.to_virtual.get(&address).map(|addr| *addr)
    }

    fn to_physical(&self, address: usize) -> Option<usize> {
        self.to_physical.get(&address).map(|addr| *addr)
    }

    unsafe fn new_table(&mut self) -> Shared<Table> {
        // calculate a new pointer and update our used
        let offset = align(self.used, 0x1000);
        self.used = offset + mem::size_of::<Table>();

        // produce physical address
        let physical_address = self.base + offset;

        // create a new table
        let table = Box::new(Table::new());

        // get the virtual address
        let virtual_address = Box::into_raw(table);
        let table = Box::from_raw(virtual_address);

        // update both mappings
        self.to_physical.insert(virtual_address as usize, physical_address);
        self.to_virtual.insert(physical_address, virtual_address as usize);

        // insert our object into our store
        self.tables.push((table, physical_address));

        Shared::new(virtual_address)
    }

    fn clear(&mut self) {
        self.used = 0;
        self.tables.clear();
        self.to_physical.clear();
        self.to_virtual.clear();
    }
}

impl RelativeBuilder {
    fn new(base: usize) -> RelativeBuilder {
        assert!(is_aligned(base, 0x1000), "Base address was not page aligned");

        RelativeBuilder {
            base: base,
            used: 0,
            tables: vec![],
            to_physical: BTreeMap::new(),
            to_virtual: BTreeMap::new()
        }
    }

    fn into_buffer(self) -> Vec<u8> {
        unsafe {
            // merge everything into one buffer
            let mut buffer: Vec<u8> = Vec::with_capacity(self.used);

            ptr::write_bytes(buffer.as_mut_ptr(), 0, buffer.capacity());

            for (table, physical_address) in self.tables {
                ptr::write(buffer.as_mut_ptr().offset((physical_address - self.base) as isize) as *mut Table,
                           *table.clone())
            }

            buffer.set_len(self.used);

            buffer
        }
    }
}

impl Layout {
    pub fn new() -> Layout {
        Layout {
            map: BTreeSet::new(),
            free: Allocator::new()
        }
    }

    unsafe fn get_or_create(&mut self, builder: &mut Base, table: &mut Table,
                            idx: usize, level: Level) -> Shared<Table> {
        let entry = table.read(idx);
        if entry.present() {
            Shared::new(
                builder.to_virtual(entry.address(level))
                    .expect("Failed to translate physical address to table address")
                    as *mut Table)
        } else {
            let new_table = builder.new_table();
            let info = Info {
                page: false,
                write: true,
                execute: true,
                user: true,
                global: false,
                write_through: false,
                cache_disable: false,
                attribute_table: false,
                protection_key: 0,
                level: level,
                address: builder.to_physical(*new_table as usize)
                    .expect("Failed to translate table address to physical address")
            };

            trace!("New table {:?}, 0x{:x}, 0x{:x}", level, idx, info.address);
            table.write(info.into(), idx);
            new_table
        }
    }

    fn build_page_at(&mut self, table: &mut Table, base: usize,
                     size: FrameSize, idx: usize, segment: &Segment) {
        let subframe_base = base + (idx * size.get_pagesize() as usize);

        let info = Info {
            page: true,
            write: segment.write(),
            execute: segment.execute(),
            user: segment.user(),
            global: segment.global(),
            write_through: false,
            cache_disable: false,
            attribute_table: false,
            protection_key: 0,
            level: match size.get_pagesize() {
                PageSize::Huge => Level::PDPTE,
                PageSize::Big => Level::PDE,
                PageSize::Page => Level::PTE
            },
            address: segment.get_physical_subframe(subframe_base)
        };

        trace!("Built page {:?}, 0x{:x}, 0x{:x}", info.level, idx, segment.get_physical_subframe(subframe_base));

        let result = table.write(info.into(), idx);

        debug_assert!(!result.present(), "Overwrote a present entry: 0x{:x}", result.address(Level::PTE));
    }

    fn build_edge(&mut self, builder: &mut Base, table: &mut Table, base: usize, size: FrameSize,
                             idx: usize, vbase: usize, segment: &Segment) -> bool {
        trace!("0x{:x}, 0x{:x}, {:?}", idx, vbase, size);

        if let Some(next) = size.get_next() {
            let subframe_base = base + (idx * size.get_pagesize() as usize);
            trace!("c 0x{:x}, 0x{:x}", subframe_base, vbase);
            if !is_aligned(segment.get_physical_subframe(subframe_base), size.get_pagesize() as usize)
                || (vbase >= subframe_base && vbase - subframe_base >= next.get_pagesize() as usize)
                || subframe_base - vbase >= next.get_pagesize() as usize
                || ((subframe_base + size.get_pagesize() as usize) > segment.virtual_base() + segment.size()
                    && (subframe_base + size.get_pagesize() as usize - segment.virtual_base() - segment.size())
                    > next.get_pagesize() as usize)
            {
                // build a new table here
                let new_table = unsafe {self.get_or_create(builder, table, idx, match next {
                        FrameSize::Giant => Level::PML4E,
                        FrameSize::Huge => Level::PDPTE,
                        FrameSize::Big => Level::PDE
                })};

                self.build_part(builder, unsafe {new_table.as_mut().unwrap()}, subframe_base,
                                next, segment);

                return true;
            }
        }

        false
    }

    fn build_part(&mut self, builder: &mut Base, table: &mut Table, base: usize,
                  size: FrameSize, segment: &Segment) {
        trace!("0x{:x}, 0x{:x}, {:?}", table as *mut _ as usize, base, size);

        let min_idx = if base > segment.virtual_base() {
            0
        } else {
            align_back(segment.virtual_base() - base, size.get_pagesize() as usize) >> size.get_pagesize().get_shift()
        };

        let max_idx = cmp::min((align(segment.virtual_base() + segment.size() - base, size.get_pagesize() as usize)
                                >> size.get_pagesize().get_shift()), 0x200);

        trace!("0x{:x}, 0x{:x}", min_idx, max_idx);

        if max_idx == 0 || min_idx >= 0x200 {
            // cannot place segment here
            panic!("build_part called with incorret parameters");
        }

        if !self.build_edge(builder, table, base, size, min_idx, segment.virtual_base(), segment) {
            self.build_page_at(table, base, size, min_idx, segment);
        }

        trace!("a 0x{:x}, 0x{:x}, {:?}", table as *mut _ as usize, base, size);
        trace!("0x{:x}, 0x{:x}", min_idx, max_idx);

        if max_idx > min_idx + 1 {
            if !self.build_edge(builder, table, base, size, max_idx - 1,
                                align_back(segment.virtual_base() + segment.size() as usize, size.get_pagesize() as usize), segment) {
                self.build_page_at(table, base, size, max_idx - 1, segment);
            }
        }

        trace!("b 0x{:x}, 0x{:x}, {:?}", table as *mut _ as usize, base, size);

        for idx in min_idx + 1..max_idx - 1 {
            if !self.build_edge(builder, table, base, size, idx, base + (idx * size.get_pagesize() as usize), segment) {
                self.build_page_at(table, base, size, idx, segment)
            }
        }
    }

    pub fn build(&mut self, builder: &mut Base) -> usize {
        unsafe {
            let root = builder.new_table();

            self.build_at(builder, root);

            builder.to_physical(*root as usize)
                .expect("Root table had no physical mapping")
        }
    }

    pub unsafe fn build_at(&mut self, builder: &mut Base, root: Shared<Table>) {
        let segments: Vec<Segment> = self.map.iter().cloned().collect();

        for segment in segments {
            let min_idx = align_back(segment.virtual_base(), FrameSize::Giant as usize)
                >> FrameSize::Giant.get_shift();
            let max_idx = align(segment.virtual_base() + segment.size(), FrameSize::Giant as usize)
                >> FrameSize::Giant.get_shift();

            for idx in min_idx..max_idx {
                let table = self.get_or_create(builder, root.as_mut().unwrap(), idx, Level::PML4E);
                self.build_part(builder, table.as_mut().unwrap(), idx * FrameSize::Giant as usize, FrameSize::Giant, &segment);
            }
        }
    }

    pub fn build_relative(&mut self, base: usize) -> (usize, Vec<u8>) {
        let mut builder = RelativeBuilder::new(base);
        let addr = self.build(&mut builder);
        (addr, builder.into_buffer())
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
        let dummy = Segment::dummy_range(region.base(), region.size());

        // only forget a region if no segments are mapped in it
        if !self.map.contains(&dummy) {
            self.free.forget(region)
        } else {
            false
        }
    }
    
    pub fn insert(&mut self, segment: Segment) -> bool {
        if self.map.insert(segment.clone()) {
            let region = Region::new(segment.virtual_base(), segment.size());
            // may or may not already be allocated
            self.free.set_used(region);

            trace!("Inserting segment {:?}", &segment);
            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, segment: Segment) -> bool {
        let region = Region::new(segment.virtual_base(), segment.size());

        if self.free.release(region) && self.map.remove(&segment) {
            true
        } else {
            false
        }
    }

    pub fn to_physical(&self, addr: usize) -> Option<usize> {
        let dummy = Segment::dummy(addr);

        if let Some(segment) = self.map.get(&dummy) {
            // address has a mapping
            Some((addr & ((1 << CANONICAL_BITS) - 1)) - segment.virtual_base() + segment.physical_base())
        } else {
            // no mapping
            None
        }
    }

    pub fn to_virtual(&self, addr: usize) -> Option<usize> {
        // naive implementation
        for segment in self.map.iter() {
            if segment.physical_base() <= addr && addr <= segment.physical_base() + segment.size() {
                return Some(addr - segment.physical_base() + segment.virtual_base());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use include::*;
    use segment::Segment;

    use std::io::{Write};
    use std::fmt::Display;
    use log;
    use super::*;

    struct TestLogger;

    impl log::Output for TestLogger {
        fn log(&mut self, level: usize, location: &log::Location, target: &Display, message: &Display) {
            if level <= 2 {
                let _ = writeln!(::std::io::stderr(), "{} {} at {}({}): {}", target, log::level_name(level), location.file, location.line, message);
            } else {
                println!("{} {} at {}({}): {}", target, log::level_name(level), location.file, location.line, message);
            }
        }
    }

    fn before() {
        if !log::has_output() {
            log::set_output(Some(Box::new(TestLogger)))
        }
    }

    #[test]
    fn test_layout() {
        before();

        let mut layout = Layout::new();
        layout.insert(Segment::new(0x200000, 0x200000, 0x4000, false, false, false, false));

        let (addr, mut buffer) = layout.build_relative(0x180000);
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        let cap = buffer.capacity();

        assert!(cap % 8 == 0, "buffer length was not a multiple of eight");

        // forget the buffer now
        mem::forget(buffer);

        // construct an object
        let tables: Vec<u64> = unsafe {Vec::from_raw_parts(ptr as *mut u64, len / 8, cap / 8)};
        let head_idx = (addr - 0x180000) / 0x8;

        // 511th entry
        let next_entry = tables[head_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 0th entry");
        let next_addr = next_entry & (((1 << 41) - 1) << 12);
        let next_idx = (next_addr - 0x180000) / 0x8;
        
        // 510th entry
        let next_entry = tables[next_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 0th entry");
        assert!(next_entry & 0x80 == 0, "510th entry was a frame");
        let next_addr = next_entry & (((1 << 41) - 1) << 12);
        let next_idx = (next_addr - 0x180000) / 0x8;

        // 3rd entry
        let next_entry = tables[next_idx as usize + 1];
        assert!(next_entry & 0x1 == 0x1, "No 1st entry");
        assert!(next_entry & 0x80 == 0, "3rd entry was a frame");
        let next_addr = next_entry & (((1 << 41) - 1) << 12);
        let next_idx = (next_addr - 0x180000) / 0x8;

        // 4th
        let next_entry = tables[next_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 0th entry");
        let frame_addr = next_entry & (((1 << 41) - 1) << 12);
        assert!(frame_addr == 0x200000, "Frame was not at the right address: 0x{:x}", frame_addr);
    }

    #[test]
    fn test_layout_high() {
        before();

        let mut layout = Layout::new();
        layout.insert(Segment::new(0x200000, 0xffffffff80200000, 0x4000, false, false, false, false));

        let (addr, mut buffer) = layout.build_relative(0x180000);
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        let cap = buffer.capacity();

        assert!(cap % 8 == 0, "buffer length was not a multiple of eight");

        // forget the buffer now
        mem::forget(buffer);

        // construct an object
        let tables: Vec<u64> = unsafe {Vec::from_raw_parts(ptr as *mut u64, len / 8, cap / 8)};
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
        let next_entry = tables[next_idx as usize + 1];
        assert!(next_entry & 0x1 == 0x1, "No 1st entry");
        assert!(next_entry & 0x80 == 0, "1st entry was a frame");
        let next_addr = next_entry & (((1 << 41) - 1) << 12);
        let next_idx = (next_addr - 0x180000) / 0x8;

        // 4th
        let next_entry = tables[next_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 0th entry");
        let frame_addr = next_entry & (((1 << 41) - 1) << 12);
        assert!(frame_addr == 0x200000, "Frame was not at the right address: 0x{:x}", frame_addr);
    }

    #[test]
    fn test_walk_data() {
        before();

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

        let (addr, mut buffer) = layout.build_relative(0x180000);
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        let cap = buffer.capacity();

        assert!(cap % 8 == 0, "buffer length was not a multiple of eight");

        // forget the buffer now
        mem::forget(buffer);

        // construct an object
        let tables: Vec<u64> = unsafe {Vec::from_raw_parts(ptr as *mut u64, len / 8, cap / 8)};
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

}

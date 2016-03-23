use include::*;

use segment::Segment;
use allocator::{Allocator, Region};
use table::{Info, Table, Base, Level};

pub struct Layout {
    map: BTreeSet<Segment>,
    free: Allocator,
    base: Option<Box<Base>>,
    tables: Vec<Unique<Table>>,
    root: Option<Unique<Table>>
}

impl Drop for Layout {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Layout {
    pub fn new(base: Option<Box<Base>>) -> Layout {
        Layout {
            map: BTreeSet::new(),
            free: Allocator::new(),
            base: base,
            tables: vec![],
            root: None
        }
    }

    fn clear(&mut self) {
        let tables: Vec<Unique<Table>> = self.tables.drain(..).collect();

        for table in tables {
            unsafe {self.release_table(table)};
        }
        
        if let Some(table) = self.root.take() {
            unsafe {self.release_table(table)};
        }
    }

    unsafe fn allocate_table(&mut self) -> Unique<Table> {
        if let Some(ref mut base) = self.base {
            base.allocate_table()
        } else {
            Unique::new(heap::allocate(mem::size_of::<Table>(), 0x1000) as *mut Table)
        }
    }

    unsafe fn release_table(&mut self, table: Unique<Table>) {
        if let Some(ref mut base) = self.base {
            base.release_table(table)
        } else {
            heap::deallocate(*table as *mut u8, mem::size_of::<Table>(), 0x1000)
        }
    }

    fn translate_physical(&self, address: usize) -> Option<usize> {
        if let Some(ref base) = self.base {
            base.to_physical(address)
        } else {
            self.to_physical(address)
        }
    }

    fn translate_virtual(&self, address: usize) -> Option<usize> {
        if let Some(ref base) = self.base {
            base.to_virtual(address)
        } else {
            self.to_virtual(address)
        }
    }

    unsafe fn get_or_create(&mut self, table: &mut Table, idx: usize, level: Level) -> Shared<Table> {
        let entry = table.read(idx);
        if entry.present() {
            Shared::new(
                self.translate_virtual(entry.address(level))
                    .expect("Failed to translate physical address to table address")
                    as *mut Table)
        } else {
            let table = self.allocate_table();
            let shared = Shared::new(*table);
            self.tables.push(table);
            shared
        }
    }

    fn build_page_at(&mut self, table: &mut Table, base: usize, size: FrameSize,
                     idx: usize, segment: &Segment) {
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

        let result = table.write(info.into(), idx);

        debug_assert!(!result.present(), "Overwrote a present entry");
    }

    fn build_edge(&mut self, table: &mut Table, base: usize, size: FrameSize,
                             idx: usize, vbase: usize, segment: &Segment) -> bool {
        trace!("0x{:x}, 0x{:x}", idx, vbase);

        if let Some(next) = size.get_next() {
            let subframe_base = base + (idx * size.get_pagesize() as usize);
            trace!("c 0x{:x}, 0x{:x}", subframe_base, vbase);
            if !is_aligned(segment.get_physical_subframe(subframe_base), size.get_pagesize() as usize) ||
                (vbase >= subframe_base && vbase - subframe_base >= next.get_pagesize() as usize) ||
                subframe_base - vbase >= next.get_pagesize() as usize
            {
                // build a new table here
                let new_table = unsafe {self.get_or_create(table, idx, match size {
                        FrameSize::Giant => Level::PML4E,
                        FrameSize::Huge => Level::PDPTE,
                        FrameSize::Big => Level::PDE
                })};

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
                    level: match size {
                        FrameSize::Giant => Level::PML4E,
                        FrameSize::Huge => Level::PDPTE,
                        FrameSize::Big => Level::PDE
                    },
                    address: self.translate_physical(*new_table as usize)
                        .expect("Failed to translate table address to physical address")
                };

                let result = unsafe {new_table.as_mut().unwrap().write(info.into(), idx)};

                debug_assert!(!result.present(), "Overwrote a present entry");

                self.build_part(unsafe {new_table.as_mut().unwrap()}, subframe_base, next, segment);

                return true;
            }
        }

        false
    }

    fn build_part(&mut self, table: &mut Table, base: usize, size: FrameSize, segment: &Segment) {
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

        if !self.build_edge(table, base, size, min_idx, segment.virtual_base(), segment) {
            self.build_page_at(table, base, size, min_idx, segment);
        }

        trace!("a 0x{:x}, 0x{:x}, {:?}", table as *mut _ as usize, base, size);
        trace!("0x{:x}, 0x{:x}", min_idx, max_idx);

        if max_idx > min_idx + 1 {
            if !self.build_edge(table, base, size, max_idx - 1,
                                align_back(segment.virtual_base() + segment.size() as usize, size.get_pagesize() as usize), segment) {
                self.build_page_at(table, base, size, max_idx - 1, segment);
            }
        }

        trace!("b 0x{:x}, 0x{:x}, {:?}", table as *mut _ as usize, base, size);

        for idx in min_idx + 1..max_idx - 1 {
            if !self.build_edge(table, base, size, idx, base + (idx * size.get_pagesize() as usize), segment) {
                self.build_page_at(table, base, size, idx, segment)
            }
        }
    }

    pub fn build(&mut self) {
        self.clear();

        let root = unsafe {self.allocate_table()};

        let segments: Vec<Segment> = self.map.iter().cloned().collect();

        for segment in segments {
            let min_idx = align_back(segment.virtual_base(), FrameSize::Giant as usize)
                >> FrameSize::Giant.get_shift();
            let max_idx = align(segment.virtual_base() + segment.size(), FrameSize::Giant as usize)
                >> FrameSize::Giant.get_shift();

            for idx in min_idx..max_idx {
                unsafe {
                    let table = self.get_or_create(root.as_mut().unwrap(), idx, Level::PML4E);
                    self.build_part(table.as_mut().unwrap(), idx * FrameSize::Giant as usize, FrameSize::Giant, &segment);
                }
            }
        }

        self.root = Some(root);
    }

    pub fn get_entry(&mut self) -> u64 {
        if self.root.is_none() {
            self.build();
        }

        if let Some(ref root) = self.root {
            self.translate_physical(**root as usize)
                .expect("No physical mapping for root table") as u64
        } else {
            panic!("No root after building");
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

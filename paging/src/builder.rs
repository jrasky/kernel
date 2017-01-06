use super::PageSize;

use std::ptr::Shared;

use collections::{Vec, BTreeMap};

use alloc::boxed::Box;

use std::mem;
use std::ptr;

use constants::*;

use segment::Segment;
use layout::Layout;
use table::{Info, Table, Base, Level};

pub struct Builder<'a> {
    base: &'a mut Base,
    root: Shared<Table>
}

struct RelativeBase {
    base: u64,
    used: u64,
    tables: Vec<(Box<Table>, u64)>,
    to_physical: BTreeMap<u64, u64>,
    to_virtual: BTreeMap<u64, u64>
}

impl Base for RelativeBase {
    fn to_virtual(&self, address: u64) -> Option<u64> {
        self.to_virtual.get(&address).map(|addr| *addr)
    }

    fn to_physical(&self, address: u64) -> Option<u64> {
        self.to_physical.get(&address).map(|addr| *addr)
    }

    unsafe fn new_table(&mut self) -> Shared<Table> {
        // calculate a new pointer and update our used
        let offset = align(self.used, 0x1000);
        self.used = offset + mem::size_of::<Table>() as u64;

        // produce physical address
        let physical_address = self.base + offset;

        // create a new table
        let table = Box::new(Table::new());

        // get the virtual address
        let virtual_address = Box::into_raw(table);
        let table = Box::from_raw(virtual_address);

        // update both mappings
        self.to_physical.insert(virtual_address as u64, physical_address);
        self.to_virtual.insert(physical_address, virtual_address as u64);

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

impl RelativeBase {
    fn new(base: u64) -> RelativeBase {
        assert!(is_aligned(base, 0x1000), "Base address was not page aligned");

        RelativeBase {
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
            let mut buffer: Vec<u8> = Vec::with_capacity(self.used as usize);

            ptr::write_bytes(buffer.as_mut_ptr(), 0, buffer.capacity());

            for (table, physical_address) in self.tables {
                ptr::write(buffer.as_mut_ptr().offset((physical_address - self.base) as isize) as *mut Table,
                           *table.clone())
            }

            buffer.set_len(self.used as usize);

            buffer
        }
    }
}

impl<'a> Builder<'a> {
    pub unsafe fn new(base: &mut Base) -> Builder {
        let root = base.new_table();

        Builder::at(base, root)
    }

    pub unsafe fn at(base: &mut Base, root: Shared<Table>) -> Builder {
        Builder {
            base: base,
            root: root
        }
    }

    #[inline]
    fn create_page(segment: &Segment, level: Level, subframe: u64) -> Info {
        Info {
            page: true,
            write: segment.write(),
            execute: segment.execute(),
            user: segment.user(),
            global: segment.global(),
            write_through: false,
            cache_disable: false,
            attribute_table: false,
            protection_key: 0,
            level: level,
            address: segment.get_physical_subframe(subframe)
        }
    }

    #[inline]
    fn is_segment_aligned(segment: &Segment, size: PageSize) -> bool {
        let start = segment.physical_base();
        let end = segment.physical_base() + segment.size();
        let aligned_end = align(end, size.get_size());

        is_aligned(start, size.get_size()) && is_aligned(segment.virtual_base(), size.get_size())
            && size.get_next().map(|next| aligned_end - end < next.get_size()).unwrap_or(true)
    }

    #[inline]
    unsafe fn write_page(table: Shared<Table>, info: Info, idx: usize) {
        //trace!("New page {:?}, 0x{:x}, 0x{:x}", info.level, idx, info.address);
        let result = table.as_mut().unwrap().write(info.into(), idx);

        debug_assert!(!result.present(), "Overwrote a present entry: 0x{:x}", result.address(Level::PTE));
    }

    unsafe fn get_or_create(&mut self, table: Shared<Table>, idx: usize, level: Level) -> Shared<Table> {
        let entry = table.as_mut().unwrap().read(idx);
        if entry.present() {
            //trace!("Found table {:?}, 0x{:x}, 0x{:x}", level, idx, entry.address(level));
            Shared::new(
                self.base.to_virtual(entry.address(level))
                    .expect("Failed to translate physical address to table address")
                    as *mut Table)
        } else {
            let new_table = self.base.new_table();
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
                address: self.base.to_physical(*new_table as u64)
                    .expect("Failed to translate table address to physical address")
            };

            //trace!("New table {:?}, 0x{:x}, 0x{:x}", level, idx, info.address);
            table.as_mut().unwrap().write(info.into(), idx);
            new_table
        }
    }
    
    unsafe fn generate_pages(&mut self, segment: &Segment, size: PageSize) {
        let start = segment.virtual_base() >> size.get_shift();
        let end = (segment.virtual_base() + segment.size() - 1) >> size.get_shift();

        for idx in start...end {
            let subframe = idx << size.get_shift();
            //debug!("Processing subframe 0x{:x} 0x{:x} 0x{:x}", subframe, size.get_size(), idx);
            let info = Builder::create_page(segment, size.get_level(), subframe);
            self.place_page(info, subframe);
        }
    }

    unsafe fn place_page(&mut self, info: Info, subframe: u64) {
        //trace!("Placing page at subframe 0x{:x}", subframe);
        let pml4e_idx = subframe >> 39 & 0x1ff;
        let pdpte_idx = subframe >> 30 & 0x1ff;
        let pde_idx = subframe >> 21 & 0x1ff;
        let pte_idx = subframe >> 12 & 0x1ff;

        let root_table = self.root;

        let pml4e_table = self.get_or_create(root_table, pml4e_idx as usize, Level::PML4E);

        if info.level == Level::PDPTE {
            Builder::write_page(pml4e_table, info, pdpte_idx as usize);
            return;
        }

        let pdpte_table = self.get_or_create(pml4e_table, pdpte_idx as usize, Level::PDPTE);

        if info.level == Level::PDE {
            Builder::write_page(pdpte_table, info, pde_idx as usize);
            return;
        }

        let pde_table = self.get_or_create(pdpte_table, pde_idx as usize, Level::PDE);

        debug_assert!(info.level == Level::PTE, "Pages cannot be PML4E in size");

        Builder::write_page(pde_table, info, pte_idx as usize);
    }

    pub unsafe fn build(mut self, layout: &mut Layout) -> u64 {
        for segment in layout.segments() {
            if Builder::is_segment_aligned(segment, PageSize::Huge) {
                self.generate_pages(segment, PageSize::Huge);
            } else if Builder::is_segment_aligned(segment, PageSize::Big) {
                self.generate_pages(segment, PageSize::Big);
            } else {
                self.generate_pages(segment, PageSize::Page);
            }
        }

        self.base.to_physical(*self.root as u64)
            .expect("Root table had no physical mapping")
    }
}

pub fn build_layout_relative(layout: &mut Layout, base: u64) -> (u64, Vec<u8>) {
    let mut base = RelativeBase::new(base);
    let addr;

    unsafe {
        let builder = Builder::new(&mut base);
        addr = builder.build(layout);
    }

    (addr, base.into_buffer())
}

#[cfg(test)]
mod tests {
    use segment::Segment;
    use layout::Layout;

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

        let (addr, mut buffer) = super::build_layout_relative(&mut layout, 0x180000);
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
    fn test_layout_aligned() {
        before();

        let mut layout = Layout::new();
        layout.insert(Segment::new(0x200000, 0x200000, 0x200000, false, false, false, false));

        let (addr, mut buffer) = super::build_layout_relative(&mut layout, 0x180000);
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
        assert!(next_entry & 0x80 == 0x80, "3rd entry was not a frame");
        let frame_addr = next_entry & (((1 << 41) - 1) << 12);
        assert!(frame_addr == 0x200000, "Frame was not at the right address: 0x{:x}", frame_addr);
    }

    #[test]
    fn test_layout_high() {
        before();

        let mut layout = Layout::new();
        layout.insert(Segment::new(0x200000, 0xffffffff80200000, 0x4000, false, false, false, false));

        let (addr, mut buffer) = super::build_layout_relative(&mut layout, 0x180000);
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

        let (addr, mut buffer) = super::build_layout_relative(&mut layout, 0x180000);
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
        let next_entry = tables[next_idx as usize + 4];
        assert!(next_entry & 0x1 == 0x1, "No 3rd entry");
        assert!(next_entry & 0x80 == 0, "3rd entry was a frame");
        let next_addr = next_entry & (((1 << 41) - 1) << 12);
        let next_idx = (next_addr - 0x180000) / 0x8;

        // 4th
        let next_entry = tables[next_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 4th entry");
        let frame_addr = next_entry & (((1 << 41) - 1) << 12);
        assert!(frame_addr == 0x23f000, "Frame was not at the right address: 0x{:x}", frame_addr);
    }

    #[test]
    fn test_walk_data_again() {
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
        layout.insert(Segment::new(0x400000, 0x400000, 0x400000,
                                   true, true, true, false));

        let (addr, mut buffer) = super::build_layout_relative(&mut layout, 0x180000);
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
        let next_entry = tables[next_idx as usize + 0];
        assert!(next_entry & 0x1 == 0x1, "No 0th entry");
        let frame_addr = next_entry & (((1 << 41) - 1) << 12);
        assert!(frame_addr == 0x23b000, "Frame was not at the right address: 0x{:x}", frame_addr);
    }
}

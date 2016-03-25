use include::*;

use c;
use cpu;

use memory;

struct MemoryTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

#[derive(Debug)]
struct ElfSymbolTag {
    ty: u32,
    size: u32,
    num: u32,
    entsize: u32,
    shndx: u32,
}

struct StaticBuilder;

impl paging::Base for StaticBuilder {
    fn to_physical(&self, address: usize) -> Option<usize> {
        Some(address)
    }

    fn to_virtual(&self, address: usize) -> Option<usize> {
        Some(address)
    }
    
    unsafe fn new_table(&mut self) -> Shared<paging::Table> {
        panic!("Static builder tried to create a table");
    }

    fn clear(&mut self) {
        // do nothing
    }
}

pub unsafe fn parse_elf(ptr: *const u32) {
    trace!("Parsing elf tag");
    let info = (ptr as *const ElfSymbolTag).as_ref().unwrap();

    let sections =
        slice::from_raw_parts((ptr as *const ElfSymbolTag).offset(1) as *const elf::SectionHeader,
                              info.num as usize);

    let mut sum: u64 = 0;

    for section in sections {
        if section.addr >= (&c::_kernel_top as *const u8 as u64) 
            && section.flags.0 & elf::SHF_ALLOC.0 == elf::SHF_ALLOC.0
        {
            // section is allocated
            sum += section.size;
        }
    }

    info!("{:#x} bytes used by kernel", sum);
}

unsafe fn parse_cmdline(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    let cmdline = match str::from_utf8(str_slice) {
        Ok(s) => s,
        Err(e) => {
            warn!("Unable to decode boot command line: {}", e);
            return;
        }
    };

    info!("Command line: {}", cmdline);

    let mut acc = format!("");
    let mut item: Option<String> = None;

    for ch in cmdline.chars() {
        match ch {
            '=' => {
                if item.is_none() {
                    item = Some(acc);
                    acc = format!("");
                } else {
                    acc.push('=');
                }
            }
            ' ' => {
                if let Some(ref item) = item {
                    parse_command(item, &acc);
                }

                item = None;
                acc.clear();
            }
            ch => {
                acc.push(ch);
            }
        }
    }

    if let Some(ref item) = item {
        parse_command(item, &acc);
    }
}

fn parse_command(item: &String, value: &String) {
    if item != "log" {
        return;
    }

    let mut acc = format!("");
    let mut item: Option<String> = None;

    for ch in value.chars() {
        if ch == '=' {
            if item.is_none() {
                item = Some(acc);
                acc = format!("");
            } else {
                acc.push('=');
            }
        } else if ch == ',' {
            if let Ok(level) = log::to_level(acc.as_ref()) {
                trace!("{:?}", item);
                log::set_level(level, item.as_ref().map(|filter| filter.as_ref()));
            } else {
                warn!("Unknown log level: {}", acc);
            }

            acc.clear();
        } else {
            acc.push(ch);
        }
    }

    if !acc.is_empty() {
        if let Ok(level) = log::to_level(acc.as_ref()) {
            trace!("{:?}", item);
            log::set_level(level, item.as_ref().map(|filter| filter.as_ref()));
        } else {
            warn!("Unknown log level: {}", acc);
        }
    }
}

unsafe fn parse_bootloader(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    match str::from_utf8(str_slice) {
        Ok(s) => {
            info!("Booted from: {}", s);
        }
        Err(e) => {
            warn!("Unable to decode bootloader name: {}", e);
        }
    }
}

unsafe fn parse_memory(ptr: *const u32) -> Vec<(usize, usize)> {
    frame!(traces);
    // memory map
    let entry_size = *ptr.offset(2).as_ref().unwrap();
    let mut entry_ptr = ptr.offset(4) as *const MemoryTag;
    let entry_end = (entry_ptr as usize + *ptr.offset(1) as usize) as *const _;

    let image_begin = &c::_image_begin as *const _ as usize;
    let image_end = &c::_image_end as *const _ as usize;

    let mut memory_regions = vec![];

    point!(traces, "parsing memory");

    while entry_ptr < entry_end {
        let entry = entry_ptr.as_ref().unwrap();
        match entry.addr_type {
            1 => {
                info!("RAM: {:16x} - {:16x} available",
                      entry.base_addr,
                      entry.base_addr + entry.length);

                if entry.base_addr + entry.length > c::_gen_max_paddr {
                    memory_regions.push((entry.base_addr as usize,
                                        entry.length as usize));
                }
            }
            3 => {
                info!("RAM: {:16x} - {:16x} ACPI",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            4 => {
                info!("RAM: {:16x} - {:16x} reserved, preserve",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            _ => {
                info!("RAM: {:16x} - {:16x} reserved",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
        }

        entry_ptr = align(entry_ptr as usize + entry_size as usize, 8) as *const _;
    }

    memory_regions
}

fn build_initial_heap(regions: &[(usize, usize)]) -> paging::Region {
    // try to create an initial heap
    let mut initial_heap = None;

    // create a heap mapping
    for (mut base, mut size) in regions.iter().cloned() {
        if base < c::_gen_max_paddr as usize {
            size -= c::_gen_max_paddr as usize - base;
            base = c::_gen_max_paddr as usize;
        }

        if size >= 0x200000 {
            initial_heap = Some(paging::Region::new(base, 0x200000));
            break;
        }

    }

    if let Some(region) = initial_heap {
        // stage2 inserts a 2MB segment immediately following max_paddr
        // if that's what we found, return now
        if region.base() as u64 == c::_gen_max_paddr {
            debug!("Found region matching optimistic heap");
        } else {
            // otherwise create the new mapping

            let segment = paging::Segment::new(region.base(), HEAP_BEGIN, region.size(),
                                               true, false, false, false);

            let mut layout = paging::Layout::new();
            layout.insert(segment);

            unsafe {
                layout.build_at(&mut StaticBuilder, Shared::new(c::_gen_page_tables as *mut _));
            }
        }

        unsafe {
            // register our initial segment

            if let Err(e) = memory::register(HEAP_BEGIN as *mut _, region.size()) {
                panic!("Failed to register initial heap: {}", e);
            }
        }

        region
    } else {
        panic!("Failed to place initial heap");
    }
}

fn setup_memory(memory_regions: Vec<(usize, usize)>) {
    use log::Frame;
    frame!(traces, "setting up memory");
    // create initial heap
    let initial_heap = build_initial_heap(memory_regions.as_ref());

    point!(traces, "built initial heap");

    // can now use simple allocator
    memory::exit_reserved();

    point!(traces, "out of reserve memory");

    assert!(cpu::task::set_used(initial_heap));

    point!(traces, "set initial heap used");

    assert!(cpu::task::current().map(
        paging::Segment::new(initial_heap.base(), HEAP_BEGIN, 0x200000,
                             // write, !user, !execute, !global
                             true, false, false, false)));

    point!(traces, "mapped initial heap");

    // register the rest of physical memory
    for (mut base, mut size) in memory_regions {
        if base < c::_gen_max_paddr as usize + 0x200000 {
            if size < 0x200000 {
                continue;
            } else {
                size -= c::_gen_max_paddr as usize + 0x200000 - base;
                base = c::_gen_max_paddr as usize + 0x200000;
            }
        }

        trace!("registering region at 0x{:x}, size 0x{:x}", base, size);

        assert!(cpu::task::register(paging::Region::new(base, size)));
    }

    point!(traces, "done registering physical memory");
}

pub unsafe fn parse_multiboot_tags(boot_info: *const u32, boot_info_size: usize) {
    frame!(traces, "parsing multiboot tags");

    // read multiboot info
    let mut ptr: *const u32 = boot_info;

    let end: *const u32 = (ptr as usize + boot_info_size) as *const _;

    ptr = align(ptr.offset(2) as usize, 8) as *const _;

    while ptr < end {
        match *ptr.as_ref().unwrap() {
            0 => {
                trace!("end of tags");
                point!(traces, "end of tags");
                break;
            }
            1 => {
                trace!("command line tag");
                point!(traces, "command line tag");
                parse_cmdline(ptr);
            }
            2 => {
                trace!("bootloader tag");
                point!(traces, "bootloader tag");
                parse_bootloader(ptr);
            }
            6 => {
                trace!("memory tag");
                point!(traces, "memory tag");
                let memory_regions = parse_memory(ptr);

                setup_memory(memory_regions);
                trace!("Done setting up memory");
            }
            9 => {
                trace!("elf tag");
                point!(traces, "elf tag");
                parse_elf(ptr);
            }
            _ => {
                // unknown tags aren't a huge issue
                trace!("unknown tag");
                point!(traces, "unknown tag");
                trace!("Found multiboot info tag {}", *ptr.as_ref().unwrap());
            }
        }

        // advance to the next tag
        ptr = align(ptr as usize + *ptr.offset(1).as_ref().unwrap() as usize, 8) as *const _;
    }
}

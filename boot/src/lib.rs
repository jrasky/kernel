#![feature(custom_derive)]
#![feature(plugin)]
#![feature(shared)]
#![feature(unsafe_no_drop_flag)]
#![feature(alloc)]
#![feature(collections)]
#![feature(heap_api)]
#![feature(asm)]
#![plugin(serde_macros)]
#![no_std]
extern crate core as std;
extern crate alloc;
extern crate kernel_std;
extern crate rlibc;
#[macro_use]
extern crate log;
extern crate serial;
extern crate constants;
extern crate paging;
#[macro_use]
extern crate collections;
extern crate memory;
extern crate uuid;
extern crate corepack;
extern crate serde;

use std::ptr::Shared;
use std::str::FromStr;

use std::cmp;
use std::mem;
use std::slice;
use std::ptr;
use std::str;

use alloc::boxed::Box;

use uuid::Uuid;

use constants::*;

use serde::Deserialize;

use kernel_std::*;

mod boot_c;

struct WatermarkBuilder {
    base: u64,
    end: u64
}

impl paging::Base for WatermarkBuilder {
    fn to_physical(&self, address: u64) -> Option<u64> {
        Some(address)
    }

    fn to_virtual(&self, address: u64) -> Option<u64> {
        Some(address)
    }

    unsafe fn new_table(&mut self) -> Shared<paging::Table> {
        let ptr = Shared::new(self.end as *mut paging::Table);
        ptr::write(*ptr, paging::Table::new());
        trace!("Watermark table at {:?}", *ptr);
        self.end += mem::size_of::<paging::Table>() as u64;
        ptr
    }

    fn clear(&mut self) {
        self.end = self.base;
    }
}

impl WatermarkBuilder {
    unsafe fn new(base: u64) -> WatermarkBuilder {
        WatermarkBuilder {
            base: base,
            end: base
        }
    }
}

// ModuleHeader heads the modules loaded by grub.
// The actual data follows on the next page boundary.
#[derive(Debug, Serialize, Deserialize)]
struct ModuleHeader {
    magic: [u8; 16], // 0af979b7-02c3-4ca6-b354-b709bec81199
    id: [u8; 16], // unique ID for this module
    base: u64, // base vaddr for this module
    size: u64, // memory size of the module
    write: bool,
    execute: bool,
}

#[no_mangle]
pub extern "C" fn bootstrap(magic: u32, boot_info: *const c_void) -> ! {
    // early setup
    kernel_std::early_setup();

    debug!("reached bootstrap");

    // test magic
    if magic != MULTIBOOT2_MAGIC {
        panic!("Incorrect magic for multiboot: 0x{:x}", magic);
    }

    unsafe {
        // test for cpu features
        test_cpuid();
        test_long_mode();
        test_sse();

        // set up SSE
        enable_sse();
    }

    // enable memory
    memory::enable();

    // parse multiboot info
    let info = unsafe {boot_c::parse_multiboot_info(boot_info)};

    // find the optimistic heap
    let mut heap: Option<Region> = None;
    let mut pages = None;

    // figure out a base address
    let mut base = OPTIMISTIC_HEAP as u64;

    for module in info.modules.iter() {
        // don't place optimistic heap above any modules
        base = cmp::max(base, align(module.memory.base() + module.memory.size(), 0x1000));
    }

    for region in info.memory.available.iter() {
        if let Some(base) = heap {
            // find another heap for initial page tables
            if region.base() + region.size() > base.base() + base.size() &&
                region.base() + region.size() - cmp::max(base.base() + base.size(), region.base()) >= OPTIMISTIC_HEAP_SIZE as u64
            {
                // region works for page heap
                pages = Some(Region::new(align(cmp::max(base.base() + base.size(), region.base()), 0x1000), OPTIMISTIC_HEAP_SIZE as u64));

                // done
                break;
            }
        } else {
            if region.base() + region.size() > base &&
                region.base() + region.size() - cmp::max(base, region.base()) >= OPTIMISTIC_HEAP_SIZE as u64
            {
                // region works for the optimistic heap
                heap = Some(Region::new(align(cmp::max(base, region.base()), 0x1000), OPTIMISTIC_HEAP_SIZE as u64));
            }
        }
    }

    let heap = heap.expect("Could not place optimistic heap");
    let pages = pages.expect("Could not place page tables");

    debug!("Initial heap: {:?}", heap);
    debug!("Page tables: {:?}", pages);

    // - create the initial page tables
    let mut layout = paging::Layout::new();

    // add the identity mapping
    layout.insert(paging::Segment::new(
        0x0, 0x0, IDENTITY_END as u64,
        true, false, true, false
    ));

    // add the optimistic heap
    layout.insert(paging::Segment::new(
        heap.base(), HEAP_BEGIN, OPTIMISTIC_HEAP_SIZE as u64,
        true, false, false, false
    ));

    // parse modules
    for module in info.modules.iter() {
        let bytes: &[u8] = unsafe {slice::from_raw_parts(module.memory.base() as *const u8, module.memory.size() as usize)};
        let header: ModuleHeader;
        let mut position: usize = 0;

        // future rust will make this lifetime boundary unnecessary
        {
            let mut de = corepack::Deserializer::new(|buf| {
                if position + buf.len() > bytes.len() {
                    Err(corepack::error::Error::simple(corepack::error::Reason::EndOfStream))
                } else {
                    unsafe {
                        ptr::copy(bytes.as_ptr().offset(position as isize), buf.as_mut_ptr(), buf.len());
                    }

                    position += buf.len();
                    Ok(())
                }
            });

            header = ModuleHeader::deserialize(&mut de).expect("Failed to decode module header");
        }

        if Uuid::from_bytes(&header.magic).expect("Failed to decode magic") != 
            Uuid::from_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap()
        {
            panic!("Provided module had invalid magic number");
        }

        debug!("Found module {}", Uuid::from_bytes(&header.id).expect("Failed to decode id"));
        position = align(position, 0x1000);
        if position < bytes.len() {
            debug!("0x{:x} bytes included", bytes.len() - position);

            // include region in page tables
            let segment = paging::Segment::new(module.memory.base() + position as u64, header.base, header.size,
                                               header.write, false, header.execute, false);
            layout.insert(segment);
        }
    }

    // create the builder
    let mut builder = unsafe {
        WatermarkBuilder::new(pages.base())
    };

    // build things out
    let page_tables = layout.build(&mut builder);

    debug!("built page tables at 0x{:x}", page_tables);

    // create the boot proto
    let proto = Box::new(BootProto::create(info, heap.base()));

    // create a starting gdt
    let mut gdt = cpu::gdt::Table::new(vec![]);

    unsafe {
        // enable paging

        // load 64-bit GDT
        gdt.install();

        trace!("installed gdt");

        // update selectors
        asm!(concat!(
            "mov ax, 0x10;",
            "mov ds, ax;",
            "mov es, ax;",
            "mov fs, ax;",
            "mov gs, ax;",
            "mov ss, ax;")
             ::: "ax" : "intel"
        );

        trace!("updated selectors");

        // far jump to long mode
    }

    // leak gdt here to avoid trying to reclaim that space
    mem::forget(gdt);

    unreachable!("bootstrap tried to return");
}

unsafe fn test_cpuid() {
    let mut flags: u32;
    let test_flags: u32;

    asm!("pushfd; pop $0" : "=r"(flags) ::: "intel");

    test_flags = flags;

    flags ^= 1 << 21;

    asm!("push $0; popfd; pushfd; pop $0" : "=r"(flags) : "0"(flags) :: "intel");

    asm!("push $0; popfd" :: "r"(test_flags) :: "intel");

    if test_flags == flags {
        panic!("No cpuid available");
    }
}

unsafe fn test_long_mode() {
    let mut cpuid_a: u32 = 0x80000000;

    asm!("cpuid" : "={eax}"(cpuid_a) : "{eax}"(cpuid_a) : "ebx", "ecx", "edx" : "intel");

    if cpuid_a < 0x80000001 {
        panic!("No long mode available");
    }

    cpuid_a = 0x80000001;
    let cpuid_d: u32;

    asm!("cpuid" : "={edx}"(cpuid_d) : "{eax}"(cpuid_a) : "ebx", "ecx" : "intel");

    if cpuid_d & 1 << 29 == 0 {
        panic!("No long mode available");
    }

    if cpuid_d & 1 << 20 == 0 {
        panic!("No NX protection available");
    }

    if cpuid_d & 1 << 11 == 0 {
        panic!("No syscall instruction");
    }
}

unsafe fn test_sse() {
    let cpuid_a: u32 = 0x1;
    let cpuid_c: u32;
    let cpuid_d: u32;

    asm!("cpuid" : "={ecx}"(cpuid_c), "={edx}"(cpuid_d) : "{eax}"(cpuid_a) : "eax", "ebx" : "intel");

    if cpuid_d & 1 << 25 == 0 {
        panic!("No SSE");
    }

    if cpuid_d & 1 << 26 == 0 {
        panic!("No SSE2");
    }

    if cpuid_c & 1 << 0 == 0 {
        panic!("No SSE3");
    }

    if cpuid_d & 1 << 19 == 0 {
        panic!("No CLFLUSH");
    }

    if cpuid_d & 1 << 5 == 0 {
        panic!("No MSR");
    }

    if cpuid_d & 1 << 11 == 0 {
        panic!("No SEP");
    }

    if cpuid_d & 1 << 24 == 0 {
        panic!("No FXSAVE/FXRSTOR");
    }

    //if cpuid_c & 1 << 30 == 0 {
    //    panic!("No RDRAND/RDSEED");
    //}
}

unsafe fn enable_sse() {
    let mut cr0: u32;

    asm!("mov $0, cr0" : "=r"(cr0) ::: "intel");

    cr0 &= 0xFFFB; // clear coprocessor emulation CR0.EM
    cr0 |= 0x2; // set coprocessor monitoring CR0.MP

    asm!("mov cr0, $0" :: "r"(cr0) :: "intel");

    let mut cr4: u32;

    asm!("mov $0, cr4" : "=r"(cr4) ::: "intel");

    cr4 |= 3 << 9; // CR4.OSFXSR and CR4.OSXMMEXCPT

    asm!("mov cr4, $0" :: "r"(cr4) :: "intel");
}

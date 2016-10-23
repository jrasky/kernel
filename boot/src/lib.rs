#![feature(proc_macro)]
#![feature(plugin)]
#![feature(shared)]
#![feature(alloc)]
#![feature(collections)]
#![feature(heap_api)]
#![feature(asm)]
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
#[macro_use]
extern crate serde_derive;

use std::ptr::Shared;
use std::str::FromStr;
use std::ops::Deref;

use std::cmp;
use std::mem;
use std::slice;
use std::ptr;
use std::str;

use alloc::boxed::Box;

use uuid::Uuid;

use constants::*;

use kernel_std::*;
use kernel_std::module::{Module, Data, Placement};

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

#[no_mangle]
pub extern "C" fn bootstrap(magic: u32, boot_info: *const c_void) -> ! {
    /*****************EARLY SETUP*****************/
    kernel_std::early_setup();

    debug!("reached bootstrap");

    // test magic
    if magic != MULTIBOOT2_MAGIC {
        panic!("Incorrect magic for multiboot: 0x{:x}", magic);
    }

    // test for cpu features
    test_cpuid();
    test_long_mode();
    test_sse();

    // set up SSE
    enable_sse();

    // enable memory
    memory::enable();

    // parse multiboot info
    let info = boot_c::parse_multiboot_info(boot_info);

    /*****************PARSE MEMORY*****************/

    // find the optimistic heap
    let mut heap: Option<Region> = None;
    let mut pages = None;

    // figure out a base address for the optimistic heap
    // place it at at least OPTIMISTC_HEAP
    let mut base = OPTIMISTIC_HEAP as u64;

    for module in info.modules.iter() {
        // don't place optimistic heap under any modules
        base = cmp::max(base, align(module.memory.end(), 0x1000));
    }

    for region in info.memory.available.iter() {
        if let Some(heap) = heap {
            // clamp and align the endpoints

            // don't place the start under the end of the already chosen optimistic heap
            let start = align(cmp::max(heap.end(), region.base()), 0x1000);
            let end = align_back(region.end(), 0x1000);

            // ensure we still have a valid region
            if start > end {
                // skip this region
                continue;
            }

            // variants are ensured above
            let size = end - start;

            if size >= OPTIMISTIC_HEAP_SIZE as u64 {
                // save as the place we'll build the page tables
                pages = Some(Region::new(start, OPTIMISTIC_HEAP_SIZE as u64));

                // and we're done!
                break;
            } 
        } else {
            // we have to consider a modified region, that's clamped and aligned to our requirements

            // clamp and align the start
            let start = align(cmp::max(base, region.base()), 0x1000);
            let end = align_back(region.end(), 0x1000);

            // start might end up being before end if the region ends before OPTIMISTC_HEAP
            if start > end {
                // skip this region
                continue;
            }

            // we've already ensured that end is >= start
            let size = end - start;

            if size >= OPTIMISTIC_HEAP_SIZE as u64 {
                // use OPTIMISTIC_HEAP_SIZE instead of size because we only care to use that much
                heap = Some(Region::new(start, OPTIMISTIC_HEAP_SIZE as u64));
            }
        }
    }

    let heap = heap.expect("Could not place optimistic heap");
    let pages = pages.expect("Could not place page tables");

    debug!("Initial heap: {:?}", heap);
    debug!("Page tables: {:?}", pages);

    /*****************PARSE MODULES*****************/

    // - create the initial page tables
    let mut layout = paging::Layout::new();

    // add the identity mapping
    assert!(layout.insert(paging::Segment::new(
        0x0, 0x0, IDENTITY_END as u64,
        true, false, true, false

    )), "failed to add segment");

    trace!("0x{:x}", HEAP_BEGIN);

    // add the optimistic heap
    assert!(layout.insert(paging::Segment::new(
        heap.base(), HEAP_BEGIN, OPTIMISTIC_HEAP_SIZE as u64,
        true, false, false, false
    )), "failed to add segment");

    // parse modules
    for grub_module in info.modules.iter() {
        let bytes: &[u8] = unsafe {
            slice::from_raw_parts(grub_module.memory.base() as *const u8, grub_module.memory.size() as usize)
        };

        let module: Module = corepack::from_bytes(bytes).expect("Failed to decode module");

        if module.magic != Uuid::from_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap() {
            panic!("Provided module had invalid magic number");
        }

        debug!("Found module {}", module.id);

        for header in module.headers.iter() {
            debug!("Module declared text {}, 0x{:x} bytes", header.id, header.size);

            // look for a matching text
            let mut text = None;

            for module_text in module.texts.iter() {
                if module_text.id == header.id {
                    text = Some(module_text);
                    break;
                }
            }

            let text = text.expect("Did not find text matching header");

            let base;

            if let Placement::Absolute(addr) = header.base {
                base = addr;
            } else {
                unimplemented!();
            }

            match text.data {
                Data::Offset(offset) => {
                    trace!("Text data at offset 0x{:x}", offset);

                    // include region in page tables
                    let segment = paging::Segment::new(
                        grub_module.memory.base() + offset,
                        base,
                        header.size,
                        header.write, false, header.execute, false
                    );

                    assert!(layout.insert(segment), "failed to insert segment");
                }
                Data::Empty => {
                    warn!("Empty sections not yet implemented");
                }
                _ => unimplemented!()
            }
        }
    }

    /*****************LOAD KERNEL*****************/

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
             ::: "ax" : "intel", "volatile"
        );

        trace!("updated selectors");
    }

    // set up paging
    assert!(page_tables >> 32 == 0, "Page tables built above 4 gigabytes, somehow");
    setup_paging(page_tables as u32);

    /*****************JUMP TO KERNEL*****************/
    // bogus target address 08:0x300000
    let target: [u8; 6] = [0x00, 0x00, 0x30, 0x00, 0x08, 0x00];

    unsafe {
        // once we enable paging, the next instruction /must/ be the far jump
        // so get our cr0 ready now
        let mut cr0: u32;
        asm!("mov $0, cr0" : "=r"(cr0) ::: "intel");
        cr0 |= 1 << 31;

        asm!(concat!(
            "mov edi, $0;", // first argument to kernel main is boot proto
            "mov cr0, $1;", // come on (enable paging)
            "ljmp $2" // and slam (far jump to kernel)
        ) :: "*m"(proto.deref()), "r"(cr0), "*m"(&target) : "edi" : "intel", "volatile");
    }

    // leak gdt here to avoid trying to reclaim that space
    mem::forget(gdt);

    unreachable!("bootstrap tried to return");
}

fn setup_paging(page_tables: u32) {
    unsafe {
        // do everything but turn on paging

        // put page table address into cr3
        asm!("mov cr3, $0" :: "r"(page_tables) :: "intel", "volatile");

        // enable PAE-flag, PSE-flag, and PGE-flag in cr4
        let mut cr4: u32;

        asm!("mov $0, cr4" : "=r"(cr4) ::: "intel");
        cr4 |= 0xb << 4;
        asm!("mov cr4, $0" :: "r"(cr4) :: "intel", "volatile");

        // set long mode bit and NX bit in EFER MSR
        let mut efer_msr = util::read_msr(EFER_MSR);
        efer_msr |= 0x9 << 8;
        // set the syscall bit too
        efer_msr |= 0x1;
        util::write_msr(EFER_MSR, efer_msr);

        // set the WP bit in the cr0 register
        let mut cr0: u32;

        asm!("mov $0, cr0" : "=r"(cr0) ::: "intel");
        cr0 |= 1 << 16;
        asm!("mov cr0, $0" :: "r"(cr0) :: "intel", "volatile");
    }
}

fn test_cpuid() {
    unsafe {
        let mut flags: u32;
        let test_flags: u32;

        asm!("pushfd; pop $0" : "=r"(flags) ::: "intel");

        test_flags = flags;

        flags ^= 1 << 21;

        asm!("push $0; popfd; pushfd; pop $0" : "=r"(flags) : "0"(flags) :: "intel");

        asm!("push $0; popfd" :: "r"(test_flags) :: "intel", "volatile");

        if test_flags == flags {
            panic!("No cpuid available");
        }
    }
}

fn test_long_mode() {
    let mut cpuid_a: u32 = 0x80000000;

    unsafe {
        asm!("cpuid" : "={eax}"(cpuid_a) : "{eax}"(cpuid_a) : "ebx", "ecx", "edx" : "intel");
    }

    if cpuid_a < 0x80000001 {
        panic!("No long mode available");
    }

    cpuid_a = 0x80000001;
    let cpuid_d: u32;

    unsafe {
        asm!("cpuid" : "={edx}"(cpuid_d) : "{eax}"(cpuid_a) : "ebx", "ecx" : "intel");
    }

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

fn test_sse() {
    let cpuid_a: u32 = 0x1;
    let cpuid_c: u32;
    let cpuid_d: u32;

    unsafe {
        asm!("cpuid" : "={ecx}"(cpuid_c), "={edx}"(cpuid_d) : "{eax}"(cpuid_a) : "eax", "ebx" : "intel");
    }

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

fn enable_sse() {
    let mut cr0: u32;

    unsafe {
        asm!("mov $0, cr0" : "=r"(cr0) ::: "intel");
    }

    cr0 &= 0xFFFB; // clear coprocessor emulation CR0.EM
    cr0 |= 0x2; // set coprocessor monitoring CR0.MP

    unsafe {
        asm!("mov cr0, $0" :: "r"(cr0) :: "intel", "volatile");
    }

    let mut cr4: u32;

    unsafe {
        asm!("mov $0, cr4" : "=r"(cr4) ::: "intel");
    }

    cr4 |= 3 << 9; // CR4.OSFXSR and CR4.OSXMMEXCPT

    unsafe {
        asm!("mov cr4, $0" :: "r"(cr4) :: "intel", "volatile");
    }
}

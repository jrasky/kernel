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
extern crate byteorder;

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

use byteorder::ByteOrder;

use constants::*;

use kernel_std::*;
use kernel_std::module::{Module, Data, Placement, Partition, Type};

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

    let mut available = Allocator::new();

    for region in info.memory.available.iter() {
        // register this region with our layout
        available.register(*region);
    }

    for module in info.modules.iter() {
        // don't allocate over any modules
        available.forget(module.memory);
    }

    // don't allocate anything below 0x200000
    available.forget(Region::new(0x0, 0x200000));

    /*****************PARSE MODULES*****************/

    // - create the initial page tables
    let mut layout = paging::Layout::new();

    debug_assert!(boot_c::get_image_end() < HEAP_BEGIN, "Boot image is larger than two megabytes");

    
    let mut entry = None;

    // parse modules
    for grub_module in info.modules.iter() {
        let bytes: &[u8] = unsafe {
            slice::from_raw_parts(grub_module.memory.base() as *const u8, grub_module.memory.size() as usize)
        };

        let module: Module = corepack::from_bytes(bytes).expect("Failed to decode module");

        if module.magic != Uuid::from_str("0af979b7-02c3-4ca6-b354-b709bec81199").unwrap() {
            panic!("Provided module had invalid magic number");
        }

        debug!("Found module {}", module.identity);

        for text in module.texts.iter() {
            debug!("Module declared text {}, 0x{:x} bytes", text.id, text.size);

            // get a base address. We don't support anything here but absolute placements.
            let base = if let Placement::Absolute(addr) = text.base {
                addr
            } else {
                unimplemented!()
            };

            // check to see if this module contains an entry point
            for port in text.provides.iter() {
                if port.identity == Uuid::parse_str("b3de1342-4d70-449d-9752-3122338aa864").unwrap() {
                    debug!("Entry point found at 0x{:x}", base + port.offset);

                    // Since we're using an interrupt to jump to the kernel, its
                    // entry can be above 4 GB
                    entry = Some(base + port.offset);
                }
            }

            // map the text data into our page tables
            match text.data {
                Data::Offset { partition, offset: data_offset } => {
                    trace!("Text data in partition {} at offset 0x{:x}", partition, data_offset);

                    // figure out the actual address in memory for this partition
                    let mut offset = grub_module.memory.base() + module.size;

                    for &Partition { index, size, align: part_align } in module.partitions.iter() {
                        // align to the start of our partition
                        offset = align(offset, part_align);

                        if index < partition {
                            // not our partition, keep going
                            offset += size;
                        } else {
                            // we've found our target
                            break;
                        }
                    }

                    let write = if let Type::Data { write } = text.ty {
                        write
                    } else {
                        false
                    };

                    let execute = if let Type::Code = text.ty {
                        true
                    } else {
                        false
                    };

                    // include region in page tables
                    let segment = paging::Segment::new(
                        offset + data_offset,
                        base,
                        text.size,
                        write, false, execute, false
                    );

                    assert!(layout.insert(segment), "failed to insert segment");
                }
                Data::Empty => {
                    let place = available.allocate(text.size, 0x1000).expect("Could not allocate space for empty region");

                    let write = if let Type::Data { write } = text.ty {
                        write
                    } else {
                        false
                    };

                    let execute = if let Type::Code = text.ty {
                        true
                    } else {
                        false
                    };

                    let segment = paging::Segment::new(
                        place.base(), base, text.size,
                        write, false, execute, false
                    );

                    assert!(layout.insert(segment), "failed to insert segment");
                }
                _ => unimplemented!()
            }
        }
    }

    let heap = available.allocate(OPTIMISTIC_HEAP_SIZE as u64, 0x1000).expect("Could not place optimistic heap");
    let pages = available.allocate(OPTIMISTIC_HEAP_SIZE as u64, 0x1000).expect("Could not place page tables");
        
    debug!("Initial heap: {:?}", heap);
    debug!("Page tables: {:?}", pages);

    // map our image in so we can safely enable paging
    assert!(layout.insert(paging::Segment::new(
        0x0, 0x0, boot_c::get_image_end(),
        true, false, true, false

    )), "failed to add segment");

    trace!("0x{:x}", HEAP_BEGIN);

    // add the optimistic heap
    assert!(layout.insert(paging::Segment::new(
        heap.base(), HEAP_BEGIN, OPTIMISTIC_HEAP_SIZE as u64,
        true, false, false, false
    )), "failed to add segment");

    /*****************LOAD KERNEL*****************/

    // make sure we found an entry point
    let entry = entry.expect("Kernel did not contain an entry point");

    // create the builder
    let mut base = unsafe { WatermarkBuilder::new(pages.base()) };
    let builder = unsafe { paging::Builder::new(&mut base) };

    // build things out
    let page_tables = unsafe { builder.build(&mut layout) };

    debug!("built page tables at 0x{:x}", page_tables);

    // set up paging
    assert!(page_tables >> 32 == 0, "Page tables built above 4 gigabytes, somehow");
    setup_paging(page_tables as u32);

    // create the boot proto
    let proto = Box::new(BootProto::create(info, heap.base()));

    // create a starting gdt
    let tss = cpu::tss::Segment::new([None, None, None, None, None, None, None],
                                     [None, None, None], 0);

    let mut gdt = cpu::gdt::Table::new(vec![tss]);

    // enter long mode
    enable_long_mode();

    unsafe {
        // load 64-bit GDT
        gdt.install();

        // set the new task
        gdt.set_task(0);

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

    // Load an initial interrupt table
    let mut idt = cpu::idt::Table::new();

    idt.insert(0x50, cpu::idt::Descriptor::new(entry, 0));

    unsafe { idt.install() };

    debug!("installed idt");

    unsafe {
        // pull the trigger
        asm!(concat!(
            "sti;",
            "nop;",
            "int 0x50;"
        ) :: "{edi}"(Box::into_raw(proto)) :: "intel", "volatile");
    }

    // leak gdt and idt here to avoid trying to reclaim that space
    mem::forget(gdt);
    mem::forget(idt);

    unreachable!("bootstrap tried to return");
}

fn enable_long_mode() {
    unsafe {
        let mut cr0: u32;
        asm!("mov $0, cr0" : "=r"(cr0) ::: "intel");
        cr0 |= 1 << 31;

        asm!(concat!(
            "mov cr0, $0;" // enable paging
        ) :: "r"(cr0) :: "intel", "volatile");

        // check EFER to make sure LMA has been set
        let efer_msr = util::read_msr(EFER_MSR);

        assert!((efer_msr >> 10) & 0x1 == 0x1, "long mode was not enabled");

        debug!("entered long mode");
    }
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

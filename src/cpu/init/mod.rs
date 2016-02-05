use collections::Vec;

pub mod gdt;
pub mod idt;
pub mod tss;

use memory::paging;

use memory;
use cpu;
use constants::*;

#[cfg(not(test))]
extern "C" {
    static _kernel_top: u8;
    static _kernel_end: u8;
    static _bss_top: u8;
    static _stack_top: u8;
    static _rodata_top: u8;
    static _rodata_end: u8;
    static _data_top: u8;
    static _data_end: u8;
    static _godata_top: u8;
    static _godata_end: u8;

    static mut _core_pages: u64;
    
    fn _swap_pages(cr3: u64);
    fn _init_pages();

    fn _bp_handler();
    fn _gp_handler();
}

#[cfg(test)]
unsafe fn _bp_handler() {
    unreachable!("Breakpoint handler reached");
}

#[cfg(test)]
unsafe fn _gp_handler() {
    unreachable!("General protection fault handler reached");
}

/// Unsafe because dropping gdt or idt leaks a reference
pub unsafe fn setup(memory_regions: Vec<(*mut memory::Opaque, usize)>)
                    -> (gdt::Table, idt::Table, cpu::stack::Stack, paging::Layout) {
    // create a new GDT with a TSS
    let tss = tss::Segment::new([None, None, None, None, None, None, None],
                                [None, None, None], 0);

    let mut gdt = gdt::Table::new(vec![tss]);

    debug!("Created new GDT");

    // install the gdt
    gdt.install();

    debug!("Installed GDT");

    // set the task
    gdt.set_task(0);

    debug!("Set new task");

    let mut descriptors = vec![];

    descriptors.push(idt::Descriptor::placeholder()); // 0
    descriptors.push(idt::Descriptor::placeholder()); // 1
    descriptors.push(idt::Descriptor::placeholder()); // 2
    descriptors.push(idt::Descriptor::new(_bp_handler as u64, 0)); // 3
    descriptors.push(idt::Descriptor::placeholder()); // 4
    descriptors.push(idt::Descriptor::placeholder()); // 5
    descriptors.push(idt::Descriptor::placeholder()); // 6
    descriptors.push(idt::Descriptor::placeholder()); // 7
    descriptors.push(idt::Descriptor::placeholder()); // 8
    descriptors.push(idt::Descriptor::placeholder()); // 9
    descriptors.push(idt::Descriptor::placeholder()); // 10
    descriptors.push(idt::Descriptor::placeholder()); // 11
    descriptors.push(idt::Descriptor::placeholder()); // 13
    descriptors.push(idt::Descriptor::new(_gp_handler as u64, 0)); // 14
    descriptors.push(idt::Descriptor::placeholder()); // 15

    let mut idt = idt::Table::new(descriptors);

    idt.install();

    debug!("Installed IDT");

    let syscall_stack = cpu::syscall::setup();

    debug!("Set up syscalls");

    // remap the kernel
    let layout = remap_kernel(memory_regions);

    (gdt, idt, syscall_stack, layout)
}

#[cfg(not(test))]
unsafe fn remap_kernel(mut memory_regions: Vec<(*mut memory::Opaque, usize)>) -> paging::Layout {
    // set up paging
    let mut layout = paging::Layout::new();

    // map sections of the kernel

    // map I/O region
    assert!(layout.insert(paging::Segment::new(
        0, 0, 0x100000,
        true, false, false, false
    )), "Could not register I/O region");

    assert!(layout.insert(paging::Segment::new(
        &_kernel_top as *const u8 as usize,
        &_kernel_top as *const u8 as usize,
        &_kernel_end as *const u8 as usize - &_kernel_top as *const u8 as usize,
        false, false, true, false)),
            "Could not register kernel text");

    assert!(layout.insert(paging::Segment::new(
        &_bss_top as *const u8 as usize,
        &_bss_top as *const u8 as usize,
        &_stack_top as *const u8 as usize - &_bss_top as *const u8 as usize,
        true, false, false, false)),
            "Could not register kernel text");

    if &_rodata_top as *const u8 as usize != &_rodata_end as *const u8 as usize {
        assert!(layout.insert(paging::Segment::new(
            &_rodata_top as *const u8 as usize,
            &_rodata_top as *const u8 as usize,
            &_rodata_end as *const u8 as usize - &_rodata_top as *const u8 as usize,
            false, false, false, false)),
                "Could not register kernel rodata");
    }

    if &_data_top as *const u8 as usize != &_data_end as *const u8 as usize {
        assert!(layout.insert(paging::Segment::new(
            &_data_top as *const u8 as usize,
            &_data_top as *const u8 as usize,
            &_data_end as *const u8 as usize - &_data_top as *const u8 as usize,
            true, false, false, false)),
                "Could not register kernel data");
    }

    // map heap
    for (ptr, size) in memory_regions {
        assert!(layout.insert(paging::Segment::new(
            ptr as usize, ptr as usize, size,
            true, false, false, false
        )), "Could not register heap section");
    }

    // create actual page tables
    let new_cr3 = layout.build_tables();

    // load the new cr3
    unsafe {
        // save the cr3 value in a static place
        _core_pages = new_cr3;

        // enable nx in EFER
        let mut efer: u64 = cpu::read_msr(EFER_MSR);

        efer |= 1 << 11;

        cpu::write_msr(EFER_MSR, efer);

        _init_pages();

        _swap_pages(new_cr3);
    }

    layout
}

#[cfg(test)]
unsafe fn remap_kernel(_: Vec<(*mut memory::Opaque, usize)>) -> paging::Layout {
    paging::Layout::new()
}

use collections::Vec;

pub mod gdt;
pub mod idt;
pub mod tss;

use paging;

use memory;
use cpu;
use constants::*;

#[cfg(not(test))]
extern "C" {
    static _kernel_top: u8;
    static _kernel_end: u8;
    static _bss_top: u8;
    static _long_stack: u8;
    static _rodata_top: u8;
    static _rodata_end: u8;
    static _data_top: u8;
    static _data_end: u8;
    
    fn _swap_pages(cr3: u64);
    fn _init_pages();

    fn _bp_handler();
    fn _gp_handler();
    fn _pf_handler();
}

static mut CORE_PAGES: u64 = 0;

#[cfg(test)]
unsafe extern "C" fn _bp_handler() {
    unreachable!("Breakpoint handler reached");
}

#[cfg(test)]
unsafe extern "C" fn _gp_handler() {
    unreachable!("General protection fault handler reached");
}

#[cfg(test)]
unsafe extern "C" fn _pf_handler() {
    unreachable!("Page fault handler reached");
}

/// Unsafe because dropping gdt or idt leaks a reference
pub unsafe fn setup() -> (gdt::Table, idt::Table, cpu::stack::Stack) {
    trace!("Setting up cpu");

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
    descriptors.push(idt::Descriptor::placeholder()); // 12
    descriptors.push(idt::Descriptor::new(_gp_handler as u64, 0)); // 13
    descriptors.push(idt::Descriptor::new(_pf_handler as u64, 0)); // 14

    let mut idt = idt::Table::new(descriptors);

    idt.install();

    debug!("Installed IDT");

    let syscall_stack = cpu::syscall::setup();

    debug!("Set up syscalls");

    (gdt, idt, syscall_stack)
}

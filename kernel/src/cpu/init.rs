use std::sync::atomic::{Ordering, AtomicBool};

use constants::*;

use kernel_std::cpu::{gdt, tss, idt, stack};

use cpu;
use c;

static mut EARLY_IDT_BUFFER: [u64; 2 * 15 * U64_BYTES] = [0; 2 * 15 * U64_BYTES];

static SETUP_DONE: AtomicBool = AtomicBool::new(false);

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

#[cfg(test)]
unsafe extern "C" fn _bp_early_handler() {
    unreachable!("Breakpoint handler reached");
}

#[cfg(test)]
unsafe extern "C" fn _gp_early_handler() {
    unreachable!("General protection fault handler reached");
}

#[cfg(test)]
unsafe extern "C" fn _pf_early_handler() {
    unreachable!("Page fault handler reached");
}

pub unsafe fn early_setup() {
    // no logging or memory at this point

    let early_idt = [
        idt::Descriptor::placeholder(), // 0
        idt::Descriptor::placeholder(), // 1
        idt::Descriptor::placeholder(), // 2
        idt::Descriptor::new(c::_bp_early_handler as u64, 0), // 3
        idt::Descriptor::placeholder(), // 4
        idt::Descriptor::placeholder(), // 5
        idt::Descriptor::placeholder(), // 6
        idt::Descriptor::placeholder(), // 7
        idt::Descriptor::placeholder(), // 8
        idt::Descriptor::placeholder(), // 9
        idt::Descriptor::placeholder(), // 10
        idt::Descriptor::placeholder(), // 11
        idt::Descriptor::placeholder(), // 12
        idt::Descriptor::new(c::_gp_early_handler as u64, 0), // 13
        idt::Descriptor::new(c::_pf_early_handler as u64, 0), // 14
    ];

    idt::early_install(&early_idt, EARLY_IDT_BUFFER.as_mut_ptr());
}

pub fn setup_done() -> bool {
    SETUP_DONE.load(Ordering::Relaxed)
}

/// Unsafe because dropping gdt or idt leaks a reference
pub unsafe fn setup() -> (gdt::Table, idt::Table, stack::Stack) {
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

    let mut idt = idt::Table::new();

    idt.insert(0x3, idt::Descriptor::new(c::_bp_handler as u64, 0));
    idt.insert(0xd, idt::Descriptor::new(c::_gp_handler as u64, 0));
    idt.insert(0xe, idt::Descriptor::new(c::_pf_handler as u64, 0));

    idt.install();

    debug!("Installed IDT");

    let syscall_stack = cpu::syscall::setup();

    debug!("Set up syscalls");

    SETUP_DONE.store(true, Ordering::Relaxed);

    (gdt, idt, syscall_stack)
}

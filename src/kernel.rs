#![feature(lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![feature(num_bits_bytes)]
#![feature(heap_api)]
#![no_std]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate elfloader;

use collections::{Vec, String};

use core::fmt;
use core::slice;
use core::str;
use core::ptr;
use core::mem;

use alloc::raw_vec::RawVec;

use constants::*;

#[macro_use]
mod log;
mod error;
mod memory;
mod constants;
mod cpu;
mod multiboot;

// pub use since they're exported
pub use memory::{__rust_allocate,
                 __rust_deallocate,
                 __rust_reallocate,
                 __rust_reallocate_inplace,
                 __rust_usable_size};

// pub use since we want to export
pub use cpu::interrupt::{interrupt_breakpoint,
                         interrupt_general_protection_fault};

extern "C" {
    fn _bp_handler();
    fn _gp_handler();
}

extern "C" fn test_task() {
    info!("Hello from a task!");
    cpu::task::switch_core();
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main
    info!("Hello!");

    debug!("Multiboot info at: {:#x}", boot_info as usize);

    unsafe {
        multiboot::parse_multiboot_tags(boot_info);
    }

    debug!("Done parsing tags");

    // once we're done with multiboot info, we can safely exit reserve memory
    memory::exit_reserved();

    debug!("Out of reserve memory");

    // create a new GDT with a TSS
    let tss = cpu::init::tss::Segment::new([None, None, None, None, None, None, None],
                                           [None, None, None], 0);

    let mut gdt = cpu::init::gdt::Table::new(vec![tss]);

    debug!("Created new GDT");

    unsafe {
        // install the gdt
        gdt.install();

        debug!("Installed GDT");

        // set the task
        gdt.set_task(0);

        debug!("Set new task");
    }

    let mut descriptors = vec![];

    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 0
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 1
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 2
    descriptors.push(cpu::init::idt::Descriptor::new(_bp_handler as u64, 0)); // 3
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 4
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 5
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 6
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 7
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 8
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 9
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 10
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 11
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 13
    descriptors.push(cpu::init::idt::Descriptor::new(_gp_handler as u64, 0)); // 14
    descriptors.push(cpu::init::idt::Descriptor::placeholder()); // 15

    let mut idt = cpu::init::idt::Table::new(descriptors);

    debug!("Created IDT");

    unsafe {
        idt.install();

        debug!("Installed IDT");
    }

    let mut task = cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task,
                                           cpu::stack::Stack::create(0x10000));

    task.execute();

    unreachable!("kernel_main tried to return");
}

#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cold]
#[inline(never)]
#[no_mangle]
#[lang = "panic_fmt"]
pub extern "C" fn rust_begin_unwind(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // enter reserve memory
    memory::enter_reserved();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::log(0, &loc, module_path!(), msg);

    // clear interrupts and halt
    // processory must be reset to continue
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}

#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

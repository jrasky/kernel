#![feature(rc_raw)]
#![feature(box_syntax)]
#![feature(naked_functions)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![feature(heap_api)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(std_panic))]
#![cfg_attr(test, feature(recover))]
#[cfg(not(test))]
extern crate core as std;
extern crate kernel_std;
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
#[macro_use]
extern crate log;
extern crate paging;
extern crate user;
extern crate constants;
extern crate serial;
extern crate memory;

use std::mem;

use kernel_std::BootProto;
use constants::*;

mod c;
mod cpu;
mod logging;

// pub use since we want to export
#[cfg(not(test))]
pub use cpu::interrupt::{interrupt_breakpoint,
                         interrupt_general_protection_fault,
                         interrupt_page_fault,
                         early_interrupt_breakpoint,
                         early_interrupt_general_protection_fault,
                         early_interrupt_page_fault};

pub use cpu::task::load_context;

#[cfg(not(test))]
extern "C" fn test_task(mut task: cpu::task::Task) -> ! {
    info!("Hello from a task!");

    task.yield_back();

    unreachable!("task exited");
}

#[no_mangle]
#[naked]
pub unsafe extern "C" fn _start() -> ! {
    // entry point for the kernel

    asm!(concat!(
        // rdi should already have the right argument
        "and rsp, -16;",
        "call start_kernel;"
    ) : : "{rsp}"(&c::_entry_stack) : : "intel", "volatile" );

    // error out if the interrupt handler returns
    unreachable!("Entry handler returned");
}

#[no_mangle]
pub unsafe extern "C" fn start_kernel(boot_proto: u64) -> ! {
    // set up logging immediately
    kernel_std::early_setup();

    trace!("reached kernel");

    kernel_main(boot_proto)
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn kernel_main(boot_proto: u64) -> ! {
    // kernel main

    // set up early data structures
    unsafe {cpu::init::early_setup()};

    // enable memory
    memory::enable();

    // set up logging
    assert!(kernel_std::set_logger(box logging::Logger::new(log::LogLevelFilter::Trace, "".into())).is_ok());

    // say hello
    info!("Hello!");

    // get boot proto out
    let proto = BootProto::parse(boot_proto).expect("Did not receive boot proto");

    // set up allocator
    unsafe {
        memory::register(HEAP_BEGIN as *mut u8, OPTIMISTIC_HEAP_SIZE)
            .expect("Failed to register optimistic heap");
    }

    // exit reserve memory
    memory::exit_reserved();

    // set up cpu data structures and other settings
    // keep references around so we don't break things
    let (gdt, idt) = unsafe {cpu::init::setup()};

    // explicity leak gdt and idt and the syscall stack and the kernel page map
    mem::forget(gdt);
    mem::forget(idt);

    // we're done with setup
    cpu::init::setup_done();

    info!("Starting tasks");

    // start some tasks
    let mut kernel_task = unsafe { cpu::task::Task::empty() };

    let mut new_task = kernel_task.spawn(test_task, cpu::stack::Stack::new(0xf000));

    kernel_task.switch(&mut new_task);

    unreachable!("kernel_main tried to return");
}


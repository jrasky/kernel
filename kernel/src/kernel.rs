#![feature(shared)]
#![feature(naked_functions)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
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

use alloc::boxed::Box;

use std::sync::atomic::{AtomicBool, Ordering};
use std::ptr::Unique;

use std::mem;

use kernel_std::BootProto;
use constants::*;

mod c;
mod cpu;

// pub use since we want to export
#[cfg(not(test))]
pub use cpu::interrupt::{interrupt_breakpoint,
                         interrupt_general_protection_fault,
                         interrupt_page_fault,
                         early_interrupt_breakpoint,
                         early_interrupt_general_protection_fault,
                         early_interrupt_page_fault};

#[cfg(not(test))]
pub use cpu::syscall::{sysenter_handler,
                       SYSCALL_STACK};



#[cfg(not(test))]
unsafe extern "C" fn test_task() -> ! {
    info!("Hello from a task!");

    info!("Spawning another task...");

    let mut gate = cpu::task::Gate::new(vec![]);

    let task = cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, test_task_2,
                                                      cpu::stack::Stack::create(0x10000),
                                                      cpu::task::current()));

    gate.add_task(task);

    user::release();

    for x in 0..7 {
        info!("x: {}", x);
        user::release();
    }

    info!("Unblocking other task...");

    gate.finish();

    info!("Task 1 done!");
    user::exit();
}

#[cfg(not(test))]
unsafe extern "C" fn test_task_2() -> ! {
    info!("Hello from another task!");

    info!("Waiting...");

    user::wait();

    info!("Unblocked!");

    for x2 in 0..5 {
        info!("x2: {}", x2);
    }

    info!("Task 2 done!");

    user::exit();
}
/*
#[cfg(not(test))]
unsafe extern "C" fn serial_handler() -> ! {
    loop {
        info!("Got character: {:?}", serial::read());
        user::release();
    }
}
*/

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
    //log::set_output(Some(Box::new(logging::Logger::new(serial::Writer::new()))));

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
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup()};

    // explicity leak gdt and idt and the syscall stack and the kernel page map
    mem::forget(gdt);
    mem::forget(idt);
    mem::forget(syscall_stack);

    info!("Starting tasks");

    // start some tasks
    cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, test_task,
                                           cpu::stack::Stack::create(0x10000),
                                           cpu::task::current()));

    // cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, serial_handler,
    //                                        cpu::stack::Stack::create(0x10000),
    //                                        cpu::task::current()));

    // cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, test_task_entry
    //                                       cpu::stack::Stack::create(0x10000),
    //                                       cpu::task::current()));

    loop {
        match cpu::task::run_next() {
            Ok(_) | Err(cpu::task::RunNextResult::Blocked(_)) => {
                // do nothing
            },
            Err(cpu::task::RunNextResult::NoTasks) => {
                // done
                break;
            }
        }
    }

    unreachable!("kernel_main tried to return");
}


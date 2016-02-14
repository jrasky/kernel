#![feature(lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(oom)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![feature(heap_api)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(std_panic))]
#![cfg_attr(test, feature(recover))]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate elfloader;
#[macro_use]
extern crate log;
extern crate paging;

#[cfg(not(test))]
use core::fmt;
#[cfg(not(test))]
use core::mem;

#[cfg(not(test))]
use core::sync::atomic::{Ordering, AtomicUsize};

#[cfg(test)]
use std::fmt;
#[cfg(test)]
use std::mem;

#[cfg(test)]
use std::sync::atomic::{Ordering, AtomicUsize};

#[cfg(not(test))]
use alloc::boxed::Box;
#[cfg(test)]
use std::boxed::Box;

mod error;
mod memory;
mod constants;
mod cpu;
mod multiboot;
mod logging;

// pub use since they're exported
#[cfg(not(test))]
pub use memory::{__rust_allocate,
                 __rust_deallocate,
                 __rust_reallocate,
                 __rust_reallocate_inplace,
                 __rust_usable_size};

// pub use since we want to export
#[cfg(not(test))]
pub use cpu::interrupt::{interrupt_breakpoint,
                         interrupt_general_protection_fault};

#[cfg(not(test))]
pub use cpu::syscall::sysenter_handler;

#[cfg(not(test))]
extern "C" fn test_task() -> ! {
    info!("Hello from a task!");

    info!("Spawning another task...");

    let mut gate = cpu::task::Gate::new(vec![]);

    let task = cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task_2,
                                                      cpu::stack::Stack::create(0x10000)));

    gate.add_task(task);

    cpu::syscall::release();

    for x in 0..7 {
        info!("x: {}", x);
        cpu::syscall::release();
    }

    info!("Unblocking other task...");

    gate.finish();

    info!("Task 1 done!");
    cpu::syscall::exit();
}

#[cfg(not(test))]
extern "C" fn test_task_2() -> ! {
    let mut request = log::Request {
        level: 3,
        location: log::Location {
            module_path: module_path!(),
            file: file!(),
            line: line!()
        },
        target: module_path!().into(),
        message: "".into()
    };

    request.message = format!("Hello from another task!");
    cpu::syscall::log(&request);

    request.message = format!("Waiting...");
    cpu::syscall::log(&request);

    cpu::syscall::wait();

    info!("Unblocked!");

    for x2 in 0..5 {
        request.message = format!("x2: {}", x2);
        cpu::syscall::log(&request);
        cpu::syscall::release();
    }

    request.message = format!("Task 2 done!");
    cpu::syscall::log(&request);
    cpu::syscall::exit();
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main

    // enable memory
    memory::enable();

    // set up logging
    log::set_output(Some(Box::new(logging::Writer::new(logging::Color::LightGray,
                                                       logging::Color::Black))));

    // say hello
    info!("Hello!");

    // parse multiboot info
    let memory_regions = unsafe { multiboot::parse_multiboot_tags(boot_info) };

    debug!("Done parsing tags");

    // exit reserved memory
    memory::exit_reserved();

    // parse elf tags
    debug!("Out of reserve memory");

    // set up cpu data structures and other settings
    // keep references around so we don't break things
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup(memory_regions)};

    // explicity leak gdt and idt and the syscall stack and the kernel page map
    mem::forget(gdt);
    mem::forget(idt);
    mem::forget(syscall_stack);

    info!("Starting tasks");

    // start some tasks
    cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task,
                                           cpu::stack::Stack::create(0x10000)));

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

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[no_mangle]
#[lang = "panic_fmt"]
pub extern "C" fn rust_begin_unwind(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    static PANIC_COUNT: AtomicUsize = AtomicUsize::new(0);

    match PANIC_COUNT.fetch_add(1, Ordering::Relaxed) {
        0 => {
            panic_fmt(msg, file, line)
        },
        1 => {
            double_panic()
        },
        _ => {
            // give up
            panic_halt()
        }
    }
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn panic_fmt(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // enter reserve memory
    memory::enter_reserved();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::log(0, &loc, module_path!(), msg);

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn double_panic() -> ! {
    // disable memory
    memory::disable();

    logging::reserve_log("Double panic");

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn panic_halt() -> ! {
    // clear interrupts and halt
    // processory must be reset to continue
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

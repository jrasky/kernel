#![feature(lang_items)]
#![feature(num_bits_bytes)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![feature(heap_api)]
#![no_std]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate elfloader;

use core::fmt;
use core::mem;

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
pub use cpu::syscall::sysenter_handler;

extern "C" fn test_task() -> ! {
    info!("Hello from a task!");

    info!("Spawning another task...");

    let mut gate = cpu::task::Gate::new(vec![]);

    let task = cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task_2,
                                                      cpu::stack::Stack::create(0x10000)));

    gate.add_task(task);

    cpu::task::release();

    for x in 0..7 {
        info!("x: {}", x);
        cpu::task::release();
    }

    info!("Unblocking other task...");

    gate.finish();

    info!("Task 1 done!");
    cpu::task::exit();
}

extern "C" fn test_task_2() -> ! {
    info!("Hello from another task!");

    info!("Waiting...");

    cpu::task::wait();

    info!("Unblocked!");

    for x2 in 0..5 {
        info!("x2: {}", x2);
        cpu::task::release();
    }

    info!("Task 2 done!");
    cpu::task::exit();
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main
    info!("Hello!");

    // parse multiboot info
    unsafe { multiboot::parse_multiboot_tags(boot_info) };

    debug!("Done parsing tags");

    // exit reserved memory
    memory::exit_reserved();

    debug!("Out of reserve memory");

    // set up cpu data structures and other settings
    // keep references around so we don't break things
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup()};

    // explicity leak gdt and idt and the syscall stack
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

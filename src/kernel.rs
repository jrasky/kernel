#![feature(lang_items)]
#![feature(set_recovery)]
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

use core::sync::atomic::{Ordering, AtomicUsize};

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

use constants::*;

extern "C" {
    static _kernel_top: u8;
    static _kernel_end: u8;
    static _bss_top: u8;
    static _stack_top: u8;
    static _rodata_top: u8;
    static _rodata_end: u8;
    static _data_top: u8;
    static _data_end: u8;
    
    fn _swap_pages(cr3: u64);
    fn _init_pages();
}

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

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main
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
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup()};

    // explicity leak gdt and idt and the syscall stack
    mem::forget(gdt);
    mem::forget(idt);
    mem::forget(syscall_stack);

    // set up paging
    let mut layout = memory::paging::Layout::new();

    // map sections of the kernel

    // map I/O region
    assert!(layout.insert(memory::paging::Segment::new(
        0, 0, 0x100000,
        true, false, false, false
    )), "Could not register I/O region");

    assert!(layout.insert(memory::paging::Segment::new(
        &_kernel_top as *const u8 as usize,
        &_kernel_top as *const u8 as usize,
        &_kernel_end as *const u8 as usize - &_kernel_top as *const u8 as usize,
        false, false, true, false)),
            "Could not register kernel text");

    assert!(layout.insert(memory::paging::Segment::new(
        &_bss_top as *const u8 as usize,
        &_bss_top as *const u8 as usize,
        &_stack_top as *const u8 as usize - &_bss_top as *const u8 as usize,
        true, false, false, false)),
            "Could not register kernel text");

    if &_rodata_top as *const u8 as usize != &_rodata_end as *const u8 as usize {
        assert!(layout.insert(memory::paging::Segment::new(
            &_rodata_top as *const u8 as usize,
            &_rodata_top as *const u8 as usize,
            &_rodata_end as *const u8 as usize - &_rodata_top as *const u8 as usize,
            false, false, false, false)),
                "Could not register kernel rodata");
    }

    if &_data_top as *const u8 as usize != &_data_end as *const u8 as usize {
        assert!(layout.insert(memory::paging::Segment::new(
            &_data_top as *const u8 as usize,
            &_data_top as *const u8 as usize,
            &_data_end as *const u8 as usize - &_data_top as *const u8 as usize,
            true, false, false, false)),
                "Could not register kernel data");
    }

    // map heap
    for (ptr, size) in memory_regions {
        assert!(layout.insert(memory::paging::Segment::new(
            ptr as usize, ptr as usize, size,
            true, false, false, false
        )), "Could not register heap section");
    }

    // create actual page tables
    let new_cr3 = layout.build_tables();

    // load the new cr3
    unsafe {
        // enable nx in EFER
        let mut efer: u64 = cpu::read_msr(EFER_MSR);

        efer |= 1 << 11;

        cpu::write_msr(EFER_MSR, efer);

        _init_pages();

        _swap_pages(new_cr3);
    }

    // leak layout
    mem::forget(layout);

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

#[cold]
#[inline(never)]
fn double_panic() -> ! {
    // reserve log
    static LOCATION: log::Location = log::Location {
        module_path: module_path!(),
        file: file!(),
        line: line!()
    };

    log::reserve_log(0, &LOCATION, module_path!(), "Double panic");

    panic_halt();
}

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

#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

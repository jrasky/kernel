#![feature(unsafe_no_drop_flag)]
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
extern crate user;

#[cfg(not(test))]
use core::fmt;
#[cfg(not(test))]
use core::mem;
#[cfg(not(test))]
use core::slice;
#[cfg(not(test))]
use core::cmp;

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

use constants::*;

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
                         interrupt_general_protection_fault,
                         interrupt_page_fault};

#[cfg(not(test))]
pub use cpu::syscall::{sysenter_handler,
                       SYSCALL_STACK};

extern "C" {
    fn test_task_entry() -> !;

    static _gen_segments_size: u64;
    static _gen_max_paddr: u64;
    static _gen_page_tables: u64;
    static _gen_segments: u8;
}

struct PanicInfo {
    msg: Option<fmt::Arguments<'static>>,
    file: &'static str,
    line: u32
}

#[cfg(not(test))]
unsafe extern "C" fn test_task() -> ! {
    info!("Hello from a task!");

    info!("Spawning another task...");

    let mut gate = cpu::task::Gate::new(vec![]);

    let task = cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task_2,
                                                      cpu::stack::Stack::create(0x10000)));

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
    user::log(&request);

    request.message = format!("Waiting...");
    user::log(&request);

    user::wait();

    info!("Unblocked!");

    for x2 in 0..5 {
        request.message = format!("x2: {}", x2);
        user::log(&request);
        user::release();
    }

    request.message = format!("Task 2 done!");
    user::log(&request);
    user::exit();
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main

    // enable memory
    memory::enable();

    // set up logging
    log::set_output(Some(Box::new(
        logging::Writer::new(logging::Color::LightGray, logging::Color::Black))));

    // say hello
    info!("Hello!");

    // read segments
    debug!("Number of segments: {}", _gen_segments_size);

    debug!("Highest paddr: 0x{:x}", _gen_max_paddr);

    // parse multiboot info
    let memory_regions = unsafe { multiboot::parse_multiboot_tags(boot_info) };

    debug!("Done parsing tags");

    // try to create an initial heap
    let mut initial_heap = None;

    // create a heap mapping
    for (mut base, mut size) in memory_regions.iter().cloned() {
        if base < _gen_max_paddr as usize {
            size -= _gen_max_paddr as usize - base;
            base = _gen_max_paddr as usize;
        }

        if size >= 0x200000 {
            initial_heap = Some(paging::Region::new(base, 0x200000));
            break;
        }

    }

    if let Some(region) = initial_heap {
        let segment = paging::Segment::new(region.base(), HEAP_BEGIN, region.size(),
                                           true, false, false, false);

        unsafe {
            assert!(segment.build_into(_gen_page_tables as *mut _),
                    "failed to build segment");

            memory::register(HEAP_BEGIN as *mut _, region.size());
        }
    } else {
        panic!("No memory found");
    }

    let segment = paging::Segment::new(0x400000, 0x400000, 0x400000,
                                       true, true, true, false);

    unsafe {
        assert!(segment.build_into(_gen_page_tables as *mut _),
                "failed to build segment");
    }

    // create our allocators
    let mut vmem = paging::Allocator::new();
    let mut pmem = paging::Allocator::new();

    // allocate our initial heap
    assert!(vmem.set_used(paging::Region::new(HEAP_BEGIN, 0x200000)));
    assert!(pmem.set_used(initial_heap.unwrap()));

    let mut next_vaddr = HEAP_BEGIN + 0x200000;

    for (mut base, mut size) in memory_regions {
        if base < _gen_max_paddr as usize + 0x200000 {
            if size < 0x200000 {
                continue;
            } else {
                size -= _gen_max_paddr as usize + 0x200000 - base;
                base = _gen_max_paddr as usize + 0x200000;
            }
        }

        assert!(vmem.register(paging::Region::new(base, size)));

        // compute the next vaddr
        next_vaddr = align(next_vaddr + size, 0x1000);
    }

    debug!("Done building into page tables");

    // exit reserved memory
    memory::exit_reserved();

    // parse elf tags
    debug!("Out of reserve memory");

    // set up cpu data structures and other settings
    // keep references around so we don't break things
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup()};

    // explicity leak gdt and idt and the syscall stack and the kernel page map
    mem::forget(gdt);
    mem::forget(idt);
    mem::forget(syscall_stack);

    info!("Starting tasks");

    // start some tasks
    cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task,
                                           cpu::stack::Stack::create(0x10000)));

    cpu::task::add(cpu::task::Task::create(cpu::task::PrivilegeLevel::CORE, test_task_entry,
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
    static mut ORIG_PANIC: PanicInfo = PanicInfo {
        msg: None,
        file: "",
        line: 0
    };

    match PANIC_COUNT.fetch_add(1, Ordering::Relaxed) {
        0 => {
            unsafe {
                ORIG_PANIC = PanicInfo {
                    // ok to ignore lifetime because this function deviates
                    msg: Some(mem::transmute(msg)),
                    file: file,
                    line: line
                };
            }
            panic_fmt(msg, file, line)
        },
        1 => {
            unsafe {double_panic(&ORIG_PANIC, msg, file, line)}
        },
        2 => {
            triple_panic(file, line)
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
fn double_panic(original: &PanicInfo, msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // disable memory
    memory::disable();

    logging::reserve_log(
        format_args!("Double panic at {}({}): {}\nWhile processing panic at {}({}): {}",
                     file, line, msg,
                     original.file, original.line,
                     original.msg.unwrap_or(format_args!("No message"))));

    panic_halt();
}

#[cfg(not(test))]
#[cold]
#[inline(never)]
fn triple_panic(file: &'static str, line: u32) -> ! {
    // disable memory
    memory::disable();

    logging::reserve_log(format_args!("Triple panic at {}({})", file, line));

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

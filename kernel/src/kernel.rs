#![feature(btree_range)]
#![feature(collections_bound)]
#![feature(reflect_marker)]
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
#[cfg(not(test))]
extern crate core as std;
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
extern crate constants;

use include::*;
use c::*;

mod include;
mod c;
mod error;
mod memory;
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
                         interrupt_page_fault,
                         early_interrupt_breakpoint,
                         early_interrupt_general_protection_fault,
                         early_interrupt_page_fault};

#[cfg(not(test))]
pub use cpu::syscall::{sysenter_handler,
                       SYSCALL_STACK};


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
pub extern "C" fn kernel_main(boot_info: *const u32, boot_info_size: usize) -> ! {
    // kernel main
    // set up early data structures
    unsafe {cpu::init::early_setup()};

    // enable memory
    memory::enable();

    // create our tracing frame
    frame!(traces);

    // set up logging
    log::set_output(Some(Box::new(
        logging::Writer::new(logging::Color::LightGray, logging::Color::Black))));

    // say hello
    info!("Hello!");
    point!(traces, "set up logging");

    // parse multiboot info
    unsafe { multiboot::parse_multiboot_tags(boot_info, boot_info_size) };

    debug!("finished parsing multiboot info");

    point!(traces, "parsed multiboot info");

    // now out of reserve memory

    // set up cpu data structures and other settings
    // keep references around so we don't break things
    let (gdt, idt, syscall_stack) = unsafe {cpu::init::setup()};

    point!(traces, "set up cpu structures");

    // explicity leak gdt and idt and the syscall stack and the kernel page map
    mem::forget(gdt);
    mem::forget(idt);
    mem::forget(syscall_stack);

    info!("Starting tasks");

    // start some tasks
    cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, test_task,
                                           cpu::stack::Stack::create(0x10000),
                                           cpu::task::current()));

    cpu::task::add(cpu::task::Task::thread(cpu::task::PrivilegeLevel::CORE, test_task_entry,
                                           cpu::stack::Stack::create(0x10000),
                                           cpu::task::current()));

    point!(traces, "created tasks");

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

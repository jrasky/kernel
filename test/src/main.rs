#![feature(start)]
#![feature(lang_items)]
#![feature(asm)]
#![feature(collections)]
#![feature(allocator)]
#![no_std]
#![no_builtins]
#![no_main]
#![allocator]

extern crate collections;
extern crate log;
extern crate user;

use collections::String;

use core::mem;

static STR1: &'static str = "Hello from a user task!";
static STR2: &'static str = "User task done!";
static STR3: &'static str = "";
static STR4: &'static str = "test";

#[no_mangle]
pub extern fn __rust_allocate(_: usize, _: usize) -> *mut u8 {
    panic!();
}

#[no_mangle]
pub extern fn __rust_deallocate(_: *mut u8, _: usize, _: usize) {
    panic!();
}

#[no_mangle]
pub extern fn __rust_reallocate(_: *mut u8, _: usize, _: usize,
                                _: usize) -> *mut u8 {
    panic!();
}

#[no_mangle]
pub extern fn __rust_reallocate_inplace(_: *mut u8, _: usize,
                                        _: usize, _: usize) -> usize {
    panic!();
}

#[no_mangle]
pub extern fn __rust_usable_size(_: usize, _: usize) -> usize {
    panic!();
}

#[start]
#[no_mangle]
pub extern "C" fn test_task_entry() {
    unsafe {
        asm!("and rsp, -16; call main;" :::: "intel");
    }
}

#[no_mangle]
pub extern "C" fn main() {
    let mut request = log::Request {
        level: 3,
        location: log::Location {
            module_path: module_path!(),
            file: file!(),
            line: line!()
        },
        target: unsafe {
            String::from_raw_parts(STR4.as_ptr() as *mut _, STR4.len(), STR4.len())
        },
        message: unsafe {
            String::from_raw_parts(STR3.as_ptr() as *mut _, STR3.len(), STR3.len())
        }
    };

    mem::drop(request.message);

    request.message = unsafe {
        String::from_raw_parts(STR1.as_ptr() as *mut _, STR1.len(), STR1.len())
    };
    user::log(&request);

    mem::drop(request.message);

    request.message = unsafe {
        String::from_raw_parts(STR2.as_ptr() as *mut _, STR2.len(), STR2.len())
    };
    user::log(&request);
    user::exit();
}

#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cold]
#[lang = "panic_fmt"]
extern "C" fn panic_fmt() {
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}

#[no_mangle]
#[cold]
#[allow(non_snake_case)]
#[inline(never)]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

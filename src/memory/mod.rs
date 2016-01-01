use core::prelude::v1::*;

use core::ptr;

// Reserve memory
mod reserve;

static MEMORY: Manager = Manager;

pub struct Opaque;

struct Header {
    magic: u64,
    size: usize,
}

// Memory Manager
struct Manager;

impl Manager {
    unsafe fn allocate(&self, size: usize, align: usize) -> Option<*mut Opaque> {
        reserve::allocate(size, align)
    }

    unsafe fn release(&self, ptr: *mut Opaque) -> Option<usize> {
        reserve::release(ptr)
    }

    fn granularity(&self, size: usize, align: usize) -> usize {
        reserve::granularity(size, align)
    }
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Option<*mut Opaque> {
    MEMORY.allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut Opaque) -> Option<usize> {
    MEMORY.release(ptr)
}

#[inline]
pub fn granularity(size: usize, align: usize) -> usize {
    MEMORY.granularity(size, align)
}

#[no_mangle]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    unsafe {allocate(size, align).unwrap_or(ptr::null_mut()) as *mut _}
}

#[no_mangle]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, _: usize, _: usize) {
    unsafe {release(ptr as *mut _).expect("release failed");}
}

#[no_mangle]
pub extern "C" fn __rust_reallocate(_: *mut u8, _: usize, _: usize, _: usize) -> *mut u8 {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn __rust_reallocate_inplace(_: *mut u8, _: usize, _: usize, _: usize) -> usize {
    unimplemented!();
}

#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, align: usize) -> usize {
    granularity(size, align)
}

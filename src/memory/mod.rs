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

    unsafe fn grow(&self, ptr: *mut Opaque, size: usize) -> bool {
        reserve::grow(ptr, size)
    }

    unsafe fn shrink(&self, ptr: *mut Opaque, size: usize) -> bool {
        reserve::shrink(ptr, size)
    }

    unsafe fn resize(&self, ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
        reserve::resize(ptr, size, align)
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
pub unsafe fn grow(ptr: *mut Opaque, size: usize) -> bool {
    MEMORY.grow(ptr, size)
}

#[inline]
pub unsafe fn shrink(ptr: *mut Opaque, size: usize) -> bool {
    MEMORY.shrink(ptr, size)
}

#[inline]
pub unsafe fn resize(ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
    MEMORY.resize(ptr, size, align)
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
pub extern "C" fn __rust_reallocate(ptr: *mut u8, _: usize, size: usize, align: usize) -> *mut u8 {
    unsafe {resize(ptr as *mut _, size, align).unwrap_or(ptr::null_mut()) as *mut _}
}

#[no_mangle]
pub extern "C" fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> usize {
    if size > old_size {
        if unsafe {grow(ptr as *mut _, size)} {
            granularity(size, align)
        } else {
            granularity(old_size, align)
        }
    } else if size < old_size {
        if unsafe {shrink(ptr as *mut _, size)} {
            granularity(size, align)
        } else {
            granularity(old_size, align)
        }
    } else {
        // noop
        granularity(old_size, align)
    }
}

#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, align: usize) -> usize {
    granularity(size, align)
}

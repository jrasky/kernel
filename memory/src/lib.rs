#![feature(const_fn)]
#![feature(allocator)]
#![allocator]
#![no_std]
extern crate core as std;
#[macro_use]
extern crate log;
extern crate constants;
extern crate spin;

use std::fmt::Display;
use std::sync::atomic::{Ordering, AtomicBool};

use std::fmt;
use std::str;
use std::ptr;

use constants::error::Error;

// Reserve memory
mod reserve;

// Simple memory
// Identity page-table only
mod simple;

static MEMORY: Manager = Manager {
    enabled: AtomicBool::new(false),
    use_reserve: AtomicBool::new(true)
};

// Memory Manager
struct Manager {
    enabled: AtomicBool,
    use_reserve: AtomicBool
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    OutOfMemory,
    OutOfSpace,
    Disabled,
    EmptyAllocation,
    TinyBlock,
    NoPlace,
    Overlap
}

impl Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MemoryError: {}", self.description())
    }
}

impl Error for MemoryError {
    fn description(&self) -> &str {
        use ::MemoryError::*;
        match self {
            &OutOfMemory => "Out of memory",
            &OutOfSpace => "In-place realloc ran out of space",
            &Disabled => "Memory disabled",
            &EmptyAllocation => "Empty allocation",
            &TinyBlock => "Block was too small",
            &NoPlace => "Could not place memory region",
            &Overlap => "Overlap in memory region"
        }
    }
}

impl Manager {
    #[inline]
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    #[inline]
    fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    fn enable(&self) {
        self.enabled.store(true, Ordering::Relaxed);
    }

    #[inline]
    fn enabled(&self) -> Result<(), MemoryError> {
        if self.is_enabled() {
            Ok(())
        } else {
            Err(MemoryError::Disabled)
        }
    }

    unsafe fn register(&self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        try!(self.enabled());

        simple::register(ptr, size)
    }

    unsafe fn forget(&self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        try!(self.enabled());

        simple::forget(ptr, size)
    }

    unsafe fn allocate(&self, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        try!(self.enabled());

        //trace!("Allocate 0x{:x} with align 0x{:x}", size, align);

        if size == 0 {
            warn!("Tried to allocate zero bytes");
            Err(MemoryError::EmptyAllocation)
        } else {
            if self.use_reserve.load(Ordering::Relaxed) {
                reserve::allocate(size, align)
            } else {
                // try to make sure we have enough of a heap
                // lazily avoid growing the simple allocator recursively
                static TRY_GROW: AtomicBool = AtomicBool::new(true);

                if simple::hint() < size && TRY_GROW.swap(false, Ordering::Acquire) {
                    // allocate another 2MB page to the simple heap
                    unimplemented!();
                }

                simple::allocate(size, align)
            }
        }
    }

    unsafe fn release(&self, ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
        try!(self.enabled());

        if ptr.is_null() {
            // do nothing
            warn!("Tried to free a null pointer");
            Err(MemoryError::EmptyAllocation)
        } else if reserve::belongs(ptr) {
            reserve::release(ptr, size, align)
        } else {
            simple::release(ptr, size, align)
        }
    }

    unsafe fn grow(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
        try!(self.enabled());

        if reserve::belongs(ptr) {
            reserve::grow(ptr, old_size, size, align)
        } else {
            simple::grow(ptr, old_size, size, align)
        }
    }

    unsafe fn shrink(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
        try!(self.enabled());

        if reserve::belongs(ptr) {
            reserve::shrink(ptr, old_size, size, align)
        } else {
            simple::shrink(ptr, old_size, size, align)
        }
    }

    unsafe fn resize(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        try!(self.enabled());

        if reserve::belongs(ptr) {
            reserve::resize(ptr, old_size, size, align)
        } else {
            simple::resize(ptr, old_size, size, align)
        }
    }

    fn granularity(&self, size: usize, align: usize) -> usize {
        // TODO: this is actually not correct
        if self.use_reserve.load(Ordering::Relaxed) {
            reserve::granularity(size, align)
        } else {
            simple::granularity(size, align)
        }
    }

    fn hint(&self) -> usize {
        if self.use_reserve.load(Ordering::Relaxed) {
            reserve::hint()
        } else {
            simple::hint()
        }
    }

    #[inline]
    fn enter_reserved(&self) -> bool {
        self.use_reserve.swap(true, Ordering::Relaxed)
    }

    #[inline]
    fn exit_reserved(&self) -> bool {
        self.use_reserve.swap(false, Ordering::Relaxed)
    }
}

#[inline]
pub unsafe fn register(ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
    MEMORY.register(ptr, size)
}

#[inline]
#[allow(dead_code)] // included for completeness
pub unsafe fn forget(ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
    MEMORY.forget(ptr, size)
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    MEMORY.allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
    MEMORY.release(ptr, size, align)
}

#[inline]
pub unsafe fn grow(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    MEMORY.grow(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn shrink(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    MEMORY.shrink(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn resize(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    MEMORY.resize(ptr, old_size, size, align)
}

#[inline]
pub fn is_enabled() -> bool {
    MEMORY.is_enabled()
}

#[inline]
pub fn enable() {
    MEMORY.enable()
}

#[inline]
pub fn disable() {
    MEMORY.disable()
}

#[inline]
pub fn enter_reserved() -> bool {
    MEMORY.enter_reserved()
}

#[inline]
pub fn exit_reserved() -> bool {
    MEMORY.exit_reserved()
}

#[inline]
pub fn granularity(size: usize, align: usize) -> usize {
    MEMORY.granularity(size, align)
}

#[inline]
pub fn hint() -> usize {
    MEMORY.hint()
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    if !is_enabled() {
        panic!("Tried to allocate with memory disabled");
    }

    match unsafe {allocate(size, align)} {
        Ok(ptr) => {
            //trace!("Allocate at: {:?}", ptr);
            ptr as *mut _
        },
        Err(error) => {
            error!("Could not allocate: {}", error);
            ptr::null_mut()
        }
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    if !is_enabled() {
        panic!("Tried to release with memory disabled");
    }

    if let Err(e) = unsafe {release(ptr as *mut _, size, align)} {
        error!("Failed to release pointer {:?}: {}", ptr, e);
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    if !is_enabled() {
        panic!("Tried to reallocate with memory disabled");
    }

    match unsafe {resize(ptr as *mut _, old_size, size, align)} {
        Ok(new_ptr) => {
            trace!("Reallocated to: {:?}", new_ptr);
            new_ptr as *mut _
        },
        Err(e) => {
            error!("Failed to reallocate: {}", e);
            ptr::null_mut()
        }
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> usize {
    if !is_enabled() {
        panic!("Tried to reallocate inplace with memory disabled");
    }

    if size > old_size {
        if let Err(e) = unsafe {grow(ptr as *mut _, old_size, size, align)} {
            error!("Failed to reallocate inplace: {}", e);
            granularity(old_size, align)
        } else {
            granularity(size, align)
        }
    } else if size < old_size {
        if let Err(e) = unsafe {shrink(ptr as *mut _, old_size, size, align)} {
            error!("Failed to reallocate inplace: {}", e);
            granularity(old_size, align)
        } else {
            granularity(size, align)
        }
    } else {
        // noop
        granularity(old_size, align)
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_usable_size(size: usize, align: usize) -> usize {
    granularity(size, align)
}

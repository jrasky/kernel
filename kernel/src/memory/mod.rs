#[cfg(not(test))]
use core::prelude::v1::*;

#[cfg(not(test))]
use core::sync::atomic::{Ordering, AtomicBool};
#[cfg(test)]
use std::sync::atomic::{Ordering, AtomicBool};

#[cfg(not(test))]
use core::ptr;
#[cfg(test)]
use std::ptr;

use alloc;

use constants::*;

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

impl Manager {
    fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn disable(&self) {
        self.enabled.store(false, Ordering::Relaxed);
    }

    fn enable(&self) {
        // set oom handler
        alloc::oom::set_oom_handler(oom);

        self.enabled.store(true, Ordering::Relaxed);
    }

    unsafe fn register(&self, ptr: *mut u8, size: usize) -> usize {
        if self.is_enabled() {
            // don't register with reserve
            simple::register(ptr, size)
        } else {
            0
        }
    }

    unsafe fn forget(&self, ptr: *mut u8, size: usize) -> usize {
        if self.is_enabled() {
            // don't forget from reserve
            simple::forget(ptr, size)
        } else {
            0
        }
    }

    unsafe fn allocate(&self, size: usize, align: usize) -> Option<*mut u8> {
        if !self.is_enabled() {
            None
        } else if size == 0 {
            warn!("Tried to allocate zero bytes");
            Some(ptr::null_mut())
        } else {
            if self.use_reserve.load(Ordering::Relaxed) {
                reserve::allocate(size, align)
            } else {
                simple::allocate(size, align)
            }
        }
    }

    unsafe fn release(&self, ptr: *mut u8, size: usize, align: usize) -> Option<usize> {
        if !self.is_enabled() {
            return None;
        }

        if ptr.is_null() {
            // do nothing
            warn!("Tried to free a null pointer");
            return Some(0);
        }

        if reserve::belongs(ptr) {
            reserve::release(ptr, size, align)
        } else {
            simple::release(ptr, size, align)
        }
    }

    unsafe fn grow(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if reserve::belongs(ptr) {
            reserve::grow(ptr, old_size, size, align)
        } else {
            simple::grow(ptr, old_size, size, align)
        }
    }

    unsafe fn shrink(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> bool {
        if !self.is_enabled() {
            return false;
        }

        if reserve::belongs(ptr) {
            reserve::shrink(ptr, old_size, size, align)
        } else {
            simple::shrink(ptr, old_size, size, align)
        }
    }

    unsafe fn resize(&self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Option<*mut u8> {
        if !self.is_enabled() {
            return None;
        }

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
pub unsafe fn register(ptr: *mut u8, size: usize) -> usize {
    MEMORY.register(ptr, size)
}

#[inline]
#[allow(dead_code)] // included for completeness
pub unsafe fn forget(ptr: *mut u8, size: usize) -> usize {
    MEMORY.forget(ptr, size)
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Option<*mut u8> {
    MEMORY.allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut u8, size: usize, align: usize) -> Option<usize> {
    MEMORY.release(ptr, size, align)
}

#[inline]
pub unsafe fn grow(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> bool {
    MEMORY.grow(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn shrink(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> bool {
    MEMORY.shrink(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn resize(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Option<*mut u8> {
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

fn oom() -> ! {
    // disable memory
    disable();

    panic!("Out of memory");
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    if !is_enabled() {
        panic!("Tried to allocate with memory disabled");
    }

    if let Some(ptr) = unsafe {allocate(size, align)} {
        trace!("Allocated at: {:?}", ptr);
        ptr as *mut _
    } else {
        critical!("Out of memory");
        ptr::null_mut()
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_deallocate(ptr: *mut u8, size: usize, align: usize) {
    if !is_enabled() {
        panic!("Tried to release with memory disabled");
    }

    if unsafe {release(ptr as *mut _, size, align)}.is_none() {
        critical!("Failed to release pointer: {:?}", ptr);
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    if !is_enabled() {
        panic!("Tried to reallocate with memory disabled");
    }

    if let Some(new_ptr) = unsafe {resize(ptr as *mut _, old_size, size, align)} {
        trace!("Reallocated to: {:?}", new_ptr);
        new_ptr as *mut _
    } else {
        critical!("Failed to reallocate");
        ptr::null_mut()
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> usize {
    if !is_enabled() {
        panic!("Tried to reallocate inplace with memory disabled");
    }

    if size > old_size {
        if unsafe {grow(ptr as *mut _, old_size, size, align)} {
            granularity(size, align)
        } else {
            critical!("Failed to reallocate inplace");
            granularity(old_size, align)
        }
    } else if size < old_size {
        if unsafe {shrink(ptr as *mut _, old_size, size, align)} {
            granularity(size, align)
        } else {
            critical!("Failed to reallocate inplace");
            granularity(old_size, align)
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

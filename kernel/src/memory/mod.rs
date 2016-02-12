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

#[repr(C)]
pub struct Opaque;

struct Header {
    magic: u64,
    size: usize,
}

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

    unsafe fn register(&self, ptr: *mut Opaque, size: usize) -> usize {
        if self.is_enabled() {
            // don't register with reserve
            simple::register(ptr, size)
        } else {
            0
        }
    }

    unsafe fn forget(&self, ptr: *mut Opaque, size: usize) -> usize {
        if self.is_enabled() {
            // don't forget from reserve
            simple::forget(ptr, size)
        } else {
            0
        }
    }

    unsafe fn allocate(&self, size: usize, align: usize) -> Option<*mut Opaque> {
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

    unsafe fn release(&self, ptr: *mut Opaque) -> Option<usize> {
        if !self.is_enabled() {
            return None;
        }

        if ptr.is_null() {
            // do nothing
            warn!("Tried to free a null pointer");
            return Some(0);
        }

        let header = match (ptr as *mut Header).offset(-1).as_mut() {
            None => {
                error!("Failed to get header on release");
                return None;
            },
            Some(header) => header
        };

        match header.magic {
            RESERVE_MAGIC => reserve::release(ptr),
            SIMPLE_MAGIC => simple::release(ptr),
            _ => {
                error!("Tried to release invalid pointer");
                None
            }
        }
    }

    unsafe fn grow(&self, ptr: *mut Opaque, size: usize) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let header = match (ptr as *mut Header).offset(-1).as_mut() {
            None => {
                error!("Failed to get header on grow");
                return false;
            },
            Some(header) => header
        };

        match header.magic {
            RESERVE_MAGIC => reserve::grow(ptr, size),
            SIMPLE_MAGIC => simple::grow(ptr, size),
            _ => {
                error!("Tried to grow invalid pointer");
                false
            }
        }
    }

    unsafe fn shrink(&self, ptr: *mut Opaque, size: usize) -> bool {
        if !self.is_enabled() {
            return false;
        }

        let header = match (ptr as *mut Header).offset(-1).as_mut() {
            None => {
                error!("Failed to get header on shrink");
                return false;
            },
            Some(header) => header
        };

        match header.magic {
            RESERVE_MAGIC => reserve::shrink(ptr, size),
            SIMPLE_MAGIC => simple::shrink(ptr, size),
            _ => {
                error!("Tried to shrink invalid pointer");
                false
            }
        }
    }

    unsafe fn resize(&self, ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
        if !self.is_enabled() {
            return None;
        }

        let header = match (ptr as *mut Header).offset(-1).as_mut() {
            None => {
                error!("Failed to get header on resize");
                return None;
            },
            Some(header) => header
        };

        match header.magic {
            RESERVE_MAGIC => reserve::resize(ptr, size, align),
            SIMPLE_MAGIC => simple::resize(ptr, size, align),
            _ => {
                error!("Tried to resize invalid pointer");
                None
            }
        }
    }

    fn granularity(&self, size: usize, align: usize) -> usize {
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
pub unsafe fn register(ptr: *mut Opaque, size: usize) -> usize {
    MEMORY.register(ptr, size)
}

#[inline]
#[allow(dead_code)] // included for completeness
pub unsafe fn forget(ptr: *mut Opaque, size: usize) -> usize {
    MEMORY.forget(ptr, size)
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
pub extern "C" fn __rust_deallocate(ptr: *mut u8, _: usize, _: usize) {
    if !is_enabled() {
        panic!("Tried to release with memory disabled");
    }

    if unsafe {release(ptr as *mut _)}.is_none() {
        critical!("Failed to release pointer: {:?}", ptr);
    }
}

#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn __rust_reallocate(ptr: *mut u8, _: usize, size: usize, align: usize) -> *mut u8 {
    if !is_enabled() {
        panic!("Tried to reallocate with memory disabled");
    }

    if let Some(new_ptr) = unsafe {resize(ptr as *mut _, size, align)} {
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
        if unsafe {grow(ptr as *mut _, size)} {
            granularity(size, align)
        } else {
            critical!("Failed to reallocate inplace");
            granularity(old_size, align)
        }
    } else if size < old_size {
        if unsafe {shrink(ptr as *mut _, size)} {
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
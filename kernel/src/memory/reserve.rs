#[cfg(not(test))]
use core::cell::UnsafeCell;
#[cfg(not(test))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
use std::cell::UnsafeCell;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(test))]
use core::mem;
#[cfg(not(test))]
use core::ptr;
#[cfg(not(test))]
use core::slice;

#[cfg(test)]
use std::mem;
#[cfg(test)]
use std::ptr;
#[cfg(test)]
use std::slice;

#[cfg(test)]
use alloc::heap;
#[cfg(test)]
use std::u64;

use constants::*;

use super::{Opaque, Header};

#[cfg(not(test))]
extern "C" {
    static _slab: u64;
    static _slab_map: u8;
}

#[cfg(not(test))]
static RESERVE: Memory = Memory {
    inner: UnsafeCell::new(MemoryInner {
        slab: &_slab as *const _ as *mut _,
        map: &_slab_map as *const _ as *mut _
    }),
    borrowed: AtomicBool::new(false),
};

#[cfg(test)]
static RESERVE: Memory = Memory {
    inner: UnsafeCell::new(MemoryInner {
        slab: ptr::null_mut(),
        map: ptr::null_mut()
    }),
    borrowed: AtomicBool::new(false),
};

struct Memory {
    inner: UnsafeCell<MemoryInner>,
    borrowed: AtomicBool,
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}

struct MemoryInner {
    slab: *mut u64,
    map: *mut u8
}

impl Memory {
    unsafe fn borrow_mut(&self) -> &mut MemoryInner {
        if !self.borrowed.compare_and_swap(false, true, Ordering::SeqCst) {
            self.inner.get().as_mut().unwrap()
        } else {
            panic!("Attempt to multiply access reserve allocator");
        }
    }

    fn lock(&self) {
        if !self.borrowed.compare_and_swap(true, false, Ordering::SeqCst) {
            panic!("Attempt to doubly lock reserve allocator");
        }
    }
}

impl MemoryInner {
    #[cfg(test)]
    #[inline]
    unsafe fn get_slab<'a, 'b>(&'a mut self) -> (&'b mut [u64], &'b mut [u8]) {
        if self.slab.is_null() {
            self.slab = heap::allocate(RESERVE_SLAB_SIZE * U64_BYTES, U64_BYTES) as *mut _;
            self.map = heap::allocate((RESERVE_SLAB_SIZE + 7) / 8, U64_BYTES);
        }

        (slice::from_raw_parts_mut(self.slab, RESERVE_SLAB_SIZE),
         slice::from_raw_parts_mut(self.map, (RESERVE_SLAB_SIZE + 7) / 8))
    }

    #[cfg(not(test))]
    #[inline]
    unsafe fn get_slab<'a, 'b>(&'a mut self) -> (&'b mut [u64], &'b mut [u8]) {
        (slice::from_raw_parts_mut(self.slab, RESERVE_SLAB_SIZE),
         slice::from_raw_parts_mut(self.map, (RESERVE_SLAB_SIZE + 7) / 8))
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Option<*mut Opaque> {
        // round up to the number of blocks we need to allocate
        let blocks = (size + mem::size_of::<Header>() + 7) / 8;
        let (slab, map) = self.get_slab();

        let mut start: usize = 0;
        let mut acc: usize = 0;
        let mut pos: usize = 0;
        let mut subpos: usize = 0;
        loop {
            // check for oom
            if pos >= (RESERVE_SLAB_SIZE + 7) / 8 {
                error!("Failed to allocate reserve memory");
                return None;
            }

            // check for free blocks
            if map[pos] & (1 << subpos) == 0 {
                // block is free
                if acc > 0 {
                    acc += 1;
                } else if align == 0 ||
                   ((self.slab as usize) + mem::size_of::<Header>() +
                    (((pos * 8) + subpos) * 8)) & (align - 1) == 0 {
                    // address is aligned
                    start = (pos * 8) + subpos;
                    acc = 1;
                }
            } else if acc > 0 {
                acc = 0;
            }

            // check if we've found enough blocks
            if acc >= blocks {
                // we've found blocks that work, mark them as used
                pos = start / 8;
                subpos = start % 8;
                for _ in 0..blocks {
                    if subpos >= 8 {
                        subpos = 0;
                        pos += 1;
                    }

                    map[pos] |= 1 << subpos;
                    subpos += 1;
                }

                // create the header
                let header = slab.get_mut(start).unwrap() as *mut _ as *mut Header;
                let base = header.offset(1) as *mut Opaque;
                let header = header.as_mut().unwrap();

                // set header fields
                header.magic = RESERVE_MAGIC;
                header.size = size;

                // return the pointer of interest
                return Some(base);
            }

            // increment our position
            subpos += 1;

            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }
        }
    }

    unsafe fn get_header<'a, 'b>(&'a self, ptr: *mut Opaque) -> Option<&'b mut Header> {
        let header_ptr = (ptr as *mut Header).offset(-1);

        if header_ptr < self.slab as *mut _ {
            error!("Pointer was below reserve slab");
            return None;
        }

        if header_ptr > self.slab.offset(RESERVE_SLAB_SIZE as isize) as *mut Header {
            error!("Pointer was above reserve slab");
            return None;
        }

        let header = match header_ptr.as_mut() {
            None => {
                error!("Pointer was null");
                return None;
            }
            Some(header) => header,
        };

        if header.magic != RESERVE_MAGIC {
            error!("Pointer was invalid");
            return None;
        }

        Some(header)
    }

    unsafe fn release(&mut self, ptr: *mut Opaque) -> Option<usize> {
        let (_, map) = self.get_slab();

        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get pointer header on release");
                return None;
            }
            Some(header) => header,
        };

        let blocks = (header.size + mem::size_of::<Header>() + 7) / 8;

        let start = ((header as *mut _) as usize) - (self.slab as usize);

        let mut pos = start / 64;
        let mut subpos = (start / 8) % 8;

        for _ in 0..blocks {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            map[pos] &= !(1 << subpos);
            subpos += 1;
        }

        let size = header.size;

        // reset header info
        header.magic = 0;
        header.size = 0;

        // successfully freed memory
        Some(size)
    }

    unsafe fn shrink(&mut self, ptr: *mut Opaque, size: usize) -> bool {
        let (_, map) = self.get_slab();
        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get pointer header on shrink");
                return false;
            }
            Some(header) => header,
        };

        if granularity(size, 0) >= header.size {
            // nothing to do
            return true;
        }

        let start = ((header as *mut _) as usize) - (self.slab as usize);
        let end = start + mem::size_of::<Header>() + header.size;
        let new_end = start + mem::size_of::<Header>() + size;

        let mut pos = new_end / 64;
        let mut subpos = (new_end / 8) % 8;

        for _ in new_end..end {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            // mark the block sa free
            map[pos] &= !(1 << subpos);
        }

        true
    }

    unsafe fn grow(&mut self, ptr: *mut Opaque, size: usize) -> bool {
        let (_, map) = self.get_slab();
        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get pointer header on shrink");
                return false;
            }
            Some(header) => header,
        };

        if granularity(header.size, 0) >= size {
            // nothing to do
            return true;
        }

        let start = ((header as *mut _) as usize) - (self.slab as usize);
        let end = start + mem::size_of::<Header>() + header.size;

        let mut pos = end / 64;
        let mut subpos = (end / 8) % 8;

        for place in 0..size - header.size {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            if map[pos] & (1 << subpos) == 0 {
                // mark the next block as used
                map[pos] |= 1 << subpos;
            } else {
                // rollback
                pos = end / 64;
                subpos = (end / 8) % 8;
                for _ in 0..place {
                    if subpos >= 8 {
                        subpos = 0;
                        pos += 1;
                    }

                    map[pos] &= !(1 << subpos);
                    subpos += 1;
                }

                // failed to grow
                return false;
            }

            subpos += 1;
        }

        // successfully grew the allocation
        true
    }

    unsafe fn resize(&mut self,
                     ptr: *mut Opaque,
                     size: usize,
                     align: usize)
                     -> Option<*mut Opaque> {
        // check to see if the pointer is already aligned
        let header = match self.get_header(ptr) {
            None => {
                error!("Tried to resize invalid pointer");
                return None;
            }
            Some(header) => header,
        };

        if (ptr as usize) | (align - 1) == 0 {
            if size > granularity(header.size, align) {
                if self.grow(ptr, size) {
                    return Some(ptr);
                }
            } else if granularity(size, align) < header.size {
                if self.shrink(ptr, size) {
                    return Some(ptr);
                }
            } else {
                // the pointer is already aligned, and already the same size? strange...
                return Some(ptr);
            }
        }

        // otherwise we need to create a new allocation
        match self.release(ptr) {
            None => {
                error!("Failed to free pointer on resize");
                return None;
            }
            Some(_) => {}
        }

        let new_ptr = match self.allocate(size, align) {
            None => {
                // roll back
                let (_, map) = self.get_slab();
                let blocks = (header.size + mem::size_of::<Header>() + 7) / 8;
                let start = ((header as *mut _) as usize) - (self.slab as usize);
                let mut pos = start / 64;
                let mut subpos = (start / 8) % 8;

                for _ in 0..blocks {
                    if subpos >= 8 {
                        subpos = 8;
                        pos += 1;
                    }

                    map[pos] |= 1 << subpos;

                    subpos += 1;
                }

                // fail, but original allocation is still intact
                return None;
            }
            Some(new_ptr) => new_ptr,
        };

        // only one thread can ever be here at a time, so it's safe
        // to keep using the old pointer
        ptr::copy(ptr as *mut u8, new_ptr as *mut u8, header.size);

        // success!
        Some(new_ptr)
    }
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Option<*mut Opaque> {
    let result = RESERVE.borrow_mut().allocate(size, align);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn release(ptr: *mut Opaque) -> Option<usize> {
    let result = RESERVE.borrow_mut().release(ptr);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn grow(ptr: *mut Opaque, size: usize) -> bool {
    let result = RESERVE.borrow_mut().grow(ptr, size);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn shrink(ptr: *mut Opaque, size: usize) -> bool {
    let result = RESERVE.borrow_mut().shrink(ptr, size);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn resize(ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
    let result = RESERVE.borrow_mut().resize(ptr, size, align);
    RESERVE.lock();
    result
}

#[inline]
pub fn granularity(size: usize, _: usize) -> usize {
    ((size + mem::size_of::<Header>() + 7) / 8) * 8
}
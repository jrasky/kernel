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

use super::Opaque;

#[cfg(not(test))]
extern "C" {
    static _slab: u64;
    static _slab_map: u8;
}

static RESERVE: Memory = Memory {
    inner: UnsafeCell::new(MemoryInner {
        slab: [0; RESERVE_SLAB_SIZE],
        map: [0; (RESERVE_SLAB_SIZE + 7) / 8]
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
    slab: [u64; RESERVE_SLAB_SIZE],
    map: [u8; (RESERVE_SLAB_SIZE + 7) / 8]
}

impl Memory {
    #[inline]
    unsafe fn borrow_mut(&self) -> &mut MemoryInner {
        if !self.borrowed.compare_and_swap(false, true, Ordering::SeqCst) {
            self.inner.get().as_mut().unwrap()
        } else {
            panic!("Attempt to multiply access reserve allocator");
        }
    }

    #[inline]
    fn lock(&self) {
        if !self.borrowed.compare_and_swap(true, false, Ordering::SeqCst) {
            panic!("Attempt to doubly lock reserve allocator");
        }
    }

    #[inline]
    fn belongs(&self, ptr: *mut Opaque) -> bool {
        self.inner.get().as_ref().unwrap().belongs(ptr)
    }
}

impl MemoryInner {
    #[inline]
    fn belongs(&self, ptr: *mut Opaque) -> bool {
        ptr as usize > self.slab.as_ptr() as usize && ptr as usize < unsafe {self.slab.as_ptr().offset(self.slab.len())} as usize
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Option<*mut Opaque> {
        // round up to the number of blocks we need to allocate
        let blocks = (size + 7) / 8;

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
            if self.map[pos] & (1 << subpos) == 0 {
                // block is free
                if acc > 0 {
                    acc += 1;
                } else if align == 0 ||
                   ((self.slab.as_ptr() as usize) +
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

                    self.map[pos] |= 1 << subpos;
                    subpos += 1;
                }

                // create the header
                let base = self.slab.get_mut(start).unwrap() as *mut Opaque;

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

    unsafe fn release(&mut self, ptr: *mut Opaque, size: usize, _: usize) -> Option<usize> {
        let blocks = (size + 7) / 8;

        let start = (ptr as usize) - (self.slab.as_ptr() as usize);

        let mut pos = start / 64;
        let mut subpos = (start / 8) % 8;

        for _ in 0..blocks {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            self.map[pos] &= !(1 << subpos);
            subpos += 1;
        }

        // successfully freed memory
        Some(size)
    }

    unsafe fn shrink(&mut self, ptr: *mut Opaque, old_size: usize, size: usize, align: usize) -> bool {
        if granularity(old_size, align) <= size {
            // nothing to do
            return true;
        }

        let start = (ptr as usize) - (self.slab.as_ptr() as usize);
        let end = start + old_size;
        let new_end = start + size;

        let mut pos = new_end / 64;
        let mut subpos = (new_end / 8) % 8;

        for _ in new_end..end {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            // mark the block sa free
            self.map[pos] &= !(1 << subpos);
        }

        true
    }

    unsafe fn grow(&mut self, ptr: *mut Opaque, old_size: usize, size: usize, align: usize) -> bool {
        if granularity(old_size, align) >= size {
            // nothing to do
            return true;
        }

        let start = (ptr as usize) - (self.slab.as_ptr() as usize);
        let end = start + old_size;

        let mut pos = end / 64;
        let mut subpos = (end / 8) % 8;

        for place in 0..size - old_size {
            if subpos >= 8 {
                subpos = 0;
                pos += 1;
            }

            if self.map[pos] & (1 << subpos) == 0 {
                // mark the next block as used
                self.map[pos] |= 1 << subpos;
            } else {
                // rollback
                pos = end / 64;
                subpos = (end / 8) % 8;
                for _ in 0..place {
                    if subpos >= 8 {
                        subpos = 0;
                        pos += 1;
                    }

                    self.map[pos] &= !(1 << subpos);
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
                     old_size: usize,
                     size: usize,
                     align: usize)
                     -> Option<*mut Opaque> {

        if size > granularity(old_size, align) {
            if self.grow(ptr, old_size, size, align) {
                return Some(ptr);
            }
        } else if granularity(old_size, align) < old_size {
            if self.shrink(ptr, old_size, size, align) {
                return Some(ptr);
            }
        }

        // otherwise we need to create a new allocation
        match self.release(ptr, old_size, align) {
            None => {
                error!("Failed to free pointer on resize");
                return None;
            }
            Some(_) => {}
        }

        let new_ptr = match self.allocate(size, align) {
            None => {
                // roll back
                let blocks = (old_size + 7) / 8;
                let start = (ptr as usize) - (self.slab.as_ptr() as usize);
                let mut pos = start / 64;
                let mut subpos = (start / 8) % 8;

                for _ in 0..blocks {
                    if subpos >= 8 {
                        subpos = 8;
                        pos += 1;
                    }

                    self.map[pos] |= 1 << subpos;

                    subpos += 1;
                }

                // fail, but original allocation is still intact
                return None;
            }
            Some(new_ptr) => new_ptr,
        };

        // only one thread can ever be here at a time, so it's safe
        // to keep using the old pointer
        ptr::copy(ptr as *mut u8, new_ptr as *mut u8, old_size);

        // success!
        Some(new_ptr)
    }
}

#[inline]
pub fn belongs(ptr: *mut Opaque) -> bool {
    RESERVE.belongs(ptr)
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

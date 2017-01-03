use std::sync::atomic::{Ordering, AtomicBool};
use std::cell::{UnsafeCell};

use std::str;
use std::ptr;
use std::cmp;

use constants::*;

use super::MemoryError;

extern "C" {
    // extern because this needs to be 8 bytes aligned
    static _reserve_slab: u8;
}

static RESERVE: Memory = Memory {
    inner: UnsafeCell::new(MemoryInner {
        hint: U64_BYTES * RESERVE_SLAB_SIZE,
        slab: unsafe { &_reserve_slab as *const u8 as *mut u64 },
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
    hint: usize,
    slab: *mut u64,
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
    fn belongs(&self, ptr: *mut u8) -> bool {
        unsafe {self.inner.get().as_ref().unwrap().belongs(ptr)}
    }

    #[inline]
    fn hint(&self) -> usize {
        unsafe {self.inner.get().as_ref().unwrap().hint()}
    }
}

impl MemoryInner {
    #[inline]
    fn belongs(&self, ptr: *mut u8) -> bool {
        //trace!("{:?}, {:?}, {}", ptr, self.slab.as_ptr(), self.slab.len());
        (ptr as usize) >= self.slab as usize && (ptr as usize) < unsafe {self.slab.offset(RESERVE_SLAB_SIZE as isize)} as usize
    }

    #[inline]
    fn hint(&self) -> usize {
        self.hint
    }

    #[inline]
    fn is_allocated(&self, position: usize) -> Result<bool, MemoryError> {
        if position >= RESERVE_SLAB_SIZE {
            // return here so we can wrap this in a try below
            return Err(MemoryError::OutOfMemory);
        }

        // each map entry has one bit for each u64 in the slab
        let bit_position = position & 0x7;
        let index = position >> 3;

        let entry = self.map[index];
        let bit = (entry >> bit_position) & 0x1;

        // bit is set when that u64 is allocated
        Ok(bit == 1)
    }

    #[inline]
    unsafe fn set_allocated(&mut self, base: usize, count: usize) {
        for position in base..base + count {
            // get the bit offset that we want
            let bit_position = position & 0x7;
            let index = position >> 3;

            // set the bit
            self.map[index] |= 0x1 << bit_position;
        }
    }

    #[inline]
    unsafe fn set_unallocated(&mut self, base: usize, count: usize) {
        for position in base..base + count {
            // get the bit offset that we want
            let bit_position = position & 0x7;
            let index = position >> 3;

            // set the bit
            self.map[index] &= !(0x1 << bit_position);
        }
    }

    #[inline]
    fn get_position(&self, ptr: *mut u8) -> usize {
        (ptr as usize - self.slab as usize) >> 3
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        let count = get_count(size);

        let mut base = 0usize;

        for position in 0..RESERVE_SLAB_SIZE {
            if try!(self.is_allocated(position)) {
                // advance the base if we're reading an already allocated slot

                // advance to the next correctly-aligned position

                // alignment is right-shifted by three because position is
                // address / 8, also avoid aligning to zero because overflow
                base = ::constants::align(position, cmp::max(align >> 3, 1));
            } else if base < position && position - base >= count {
                // we've found our slot
                self.set_allocated(base, count);

                let pointer = self.slab.offset(base as isize * 8);

                return Ok(pointer as *mut u8);
            }
        }

        // oom
        Err(MemoryError::OutOfMemory)
    }

    unsafe fn release(&mut self, ptr: *mut u8, size: usize, _: usize) -> Result<usize, MemoryError> {
        let count = get_count(size);
        let base = self.get_position(ptr);

        self.set_unallocated(base, count);

        // successfully freed memory
        Ok(size)
    }

    unsafe fn shrink(&mut self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
        if granularity(old_size, align) <= size {
            // nothing to do
            return Ok(());
        }

        let base = self.get_position(ptr);

        let old_count = get_count(old_size);
        let count = get_count(size);

        self.set_unallocated(base + count, old_count - count);

        Ok(())
    }

    fn can_grow_inplace(&self, base: usize, old_count: usize, count: usize) -> bool {
        for position in base + old_count..base + count {
            if self.is_allocated(position).unwrap_or(true) {
                // this position is allocated or invalid
                return false;
            }
        }

        true
    }

    unsafe fn grow(&mut self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
        if granularity(old_size, align) >= size {
            // nothing to do
            return Ok(());
        }

        let base = self.get_position(ptr);
        let old_count = get_count(old_size);
        let count = get_count(size);

        // see if we can just grow this allocation directly
        if self.can_grow_inplace(base, old_count, count) {
            // just set the new stuff as allocated and done
            self.set_allocated(base + old_count, count - old_count);

            Ok(())
        } else {
            // no space to grow this allocation
            Err(MemoryError::OutOfSpace)
        }
    }

    unsafe fn resize(&mut self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        // try to resize in-place
        if size > old_size && self.grow(ptr, old_size, size, align).is_ok() {
            return Ok(ptr);
        } else if self.shrink(ptr, old_size, size, align).is_ok() {
            return Ok(ptr);
        }

        // need to create a new allocation
        let base = self.get_position(ptr);
        let old_count = get_count(old_size);

        self.set_unallocated(base, old_count);

        if let Ok(new_ptr) = self.allocate(size, align) {
            ptr::copy(ptr, new_ptr, old_size);

            Ok(new_ptr)
        } else {
            // roll back
            self.set_allocated(base, old_count);

            // oom
            Err(MemoryError::OutOfMemory)
        }
    }
}

#[inline]
fn get_count(size: usize) -> usize {
    (size + 7) >> 3
}

#[inline]
pub fn belongs(ptr: *mut u8) -> bool {
    RESERVE.belongs(ptr)
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    let result = RESERVE.borrow_mut().allocate(size, align);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn release(ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
    let result = RESERVE.borrow_mut().release(ptr, size, align);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn grow(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    let result = RESERVE.borrow_mut().grow(ptr, old_size, size, align);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn shrink(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    let result = RESERVE.borrow_mut().shrink(ptr, old_size, size, align);
    RESERVE.lock();
    result
}

#[inline]
pub unsafe fn resize(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    let result = RESERVE.borrow_mut().resize(ptr, old_size, size, align);
    RESERVE.lock();
    result
}

#[inline]
pub fn hint() -> usize {
    RESERVE.hint()
}

#[inline]
pub fn granularity(size: usize, _: usize) -> usize {
    ((size + 7) / 8) * 8
}

use std::mem;
use std::cmp;
use std::str;
use std::ptr;

use spin::Mutex;

use constants;

use super::MemoryError;

static MEMORY: Mutex<Manager> = Mutex::new(Manager::new());

#[derive(Debug, Clone, Copy)]
struct Block {
    base: *mut u8,
    end: *mut u8,
    next: *mut Block,
    last: *mut Block,
}

struct Manager {
    hint: usize,
    free: *mut Block
}

// Manager is an internally-managed singleton
unsafe impl Sync for Manager {}
unsafe impl Send for Manager {}

impl Manager {
    #[inline]
    const fn new() -> Manager {
        Manager {
            hint: 0,
            free: ptr::null_mut()
        }
    }

    #[inline]
    fn hint(&self) -> usize {
        self.hint
    }

    unsafe fn find_around(&self, around: *mut u8) -> (*mut Block, *mut Block) {
        // walk until we find the last free block before and the first free block after
        let mut pointer = self.free;

        if pointer as usize > around as usize {
            // the free list is entirely past around
            return (ptr::null_mut(), pointer);
        }

        while !pointer.is_null() {
            if pointer as usize <= around as usize {
                let next = pointer.as_ref().unwrap().next;

                if next as usize > around as usize || next.is_null() {
                    return (pointer, pointer.as_mut().unwrap().next);
                }
            }

            pointer = pointer.as_ref().unwrap().next;
        }

        (ptr::null_mut(), ptr::null_mut())
    }

    unsafe fn insert_between(&mut self, before: *mut Block, after: *mut Block, new: *mut Block) {
        if self.free.is_null() {
            self.free = new;

            let free = self.free.as_mut().unwrap();
            free.last = ptr::null_mut();
            free.next = ptr::null_mut();

            return;
        }

        debug_assert!(!new.is_null());

        if let Some(before) = before.as_mut() {
            before.next = new;
        } else {
            self.free = new;
        }

        let new = new.as_mut().unwrap();

        new.last = before;
        new.next = after;

        if let Some(after) = after.as_mut() {
            after.last = new;
        }
    }

    unsafe fn register(&mut self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        // find the blocks around the given area
        let (before, after) = self.find_around(ptr);

        // TODO check_invariants

        let new_block = (ptr as *mut Block).as_mut().unwrap();

        new_block.base = ptr;
        new_block.end = ptr.offset(size as isize);

        self.insert_between(before, after, new_block);

        // TODO coalesce

        self.hint += size;

        Ok(size)
    }

    unsafe fn forget(&mut self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        let (before, _) = self.find_around(ptr);

        let (last, after) = self.find_around(ptr.offset(size as isize));

        if last.is_null() {
            // do nothing
            return Err(MemoryError::NoPlace);
        }

        if (last.as_ref().unwrap().end as usize) < ptr as usize + size {
            // no block overlaps our region
            let before = before.as_mut().unwrap();
            let after = after.as_mut().unwrap();

            before.end = ptr;
            before.next = after;
            after.last = before;
        } else {
            // save values before they might get clobbered
            let before = before.as_mut().unwrap();

            let maybe_last = before.last;
            let end = last.as_ref().unwrap().end;

            // truncate before and last and hook them up
            let new_block = (ptr.offset(size as isize) as *mut Block).as_mut().unwrap();

            new_block.base = ptr.offset(size as isize);
            new_block.end = end;

            if before as *mut Block as usize != ptr as usize {
                before.end = ptr;

                self.insert_between(before, after, new_block);
            } else {
                self.insert_between(maybe_last, after, new_block);
            }
        }

        // TODO maybe improve this
        self.hint -= size;

        Ok(size)
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        let size = granularity(size, align);

        let mut pointer = self.free;

        while !pointer.is_null() {
            let aligned_base = constants::align(pointer as usize, align);
            let block_end = pointer.as_ref().unwrap().end as usize;

            if block_end > aligned_base && block_end - aligned_base >= size {
                self.hint -= size;
                return self.forget(constants::align(pointer as usize, align) as *mut u8, size)
                    .map(|_| constants::align(pointer as usize, align) as *mut u8)
            }

            pointer = pointer.as_ref().unwrap().next;
        }

        Err(MemoryError::OutOfMemory)
    }

    unsafe fn release(&mut self, ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
        let size = granularity(size, align);
        let registered_size = try!(self.register(ptr, size));
        trace!("{}", registered_size);

        if registered_size == size {
            self.hint += size;
            Ok(size)
        } else {
            Err(MemoryError::Overlap)
        }
    }

    unsafe fn grow(&mut self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
        let size = granularity(size, align);
        let old_size = granularity(old_size, align);

        let (before, _) = self.find_around(ptr.offset(old_size as isize));

        if before as usize != ptr as usize {
            return Err(MemoryError::OutOfSpace)
        }

        let before = before.as_mut().unwrap();

        if before.end < ptr.offset(size as isize) {
            return Err(MemoryError::OutOfSpace)
        }

        self.forget(ptr.offset(old_size as isize), size - old_size).map(|_| ())
    }

    unsafe fn shrink(&mut self, ptr: *mut u8, old_size: usize, mut size: usize, align: usize) -> Result<(), MemoryError> {
        // adjust size
        size = granularity(size, align);
        
        if size >= granularity(old_size, align) {
            return Ok(());
        }

        let difference = size - old_size;
        let registered_size = try!(self.register((ptr as *mut u8).offset(size as isize) as *mut u8, difference));

        trace!("f: {}, {}", difference, registered_size);

        if registered_size != difference {
            return Err(MemoryError::Overlap);
        } else {
            return Ok(());
        }
    }

    unsafe fn resize(&mut self, ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        trace!("Resizing at {:?} to 0x{:x} with align 0x{:x}", ptr, size, align);

        if (ptr as usize) & (align - 1) == 0 {
            // pointer is already aligned
            trace!("Trying inplace");
            if granularity(size, align) > granularity(old_size, align) {
                trace!("Growing");
                if self.grow(ptr, old_size, size, align).is_ok() {
                    return Ok(ptr);
                }
            } else if granularity(size, align) < granularity(old_size, align) {
                trace!("Shrinking");
                if self.shrink(ptr, old_size, size, align).is_ok() {
                    return Ok(ptr);
                }
            } else {
                // pointer is aligned and the right size, do nothing
                trace!("Doing nothing");
                return Ok(ptr);
            }
        }

        // keep data that might be clobbered by release
        let diff_size: usize = cmp::min(mem::size_of::<Block>(), size);
        let mut store: Block = mem::zeroed();
        trace!("Copying: 0x{:x}", diff_size);
        ptr::copy(ptr as *mut u8, (&mut store as *mut _ as *mut u8), diff_size);

        trace!("0x{:x}", diff_size);

        if let Err(e) = self.release(ptr, old_size, align) {
            error!("Failed to free pointer on resize: {}", e);
            return Err(e);
        }

        if let Ok(new_ptr) = self.allocate(size, align) {
            trace!("{:?}, {:?}, 0x{:x}", ptr, new_ptr, old_size);

            // copy the data from the old pointer
            ptr::copy(ptr as *mut u8, new_ptr as *mut u8, old_size);

            trace!("{:?}, 0x{:x}", (&mut store as *mut _ as *mut u8), diff_size);

            // some bytes at the beginning might have been clobbered
            // copy data that might have been clobbered
            ptr::copy((&mut store as *mut _ as *mut u8), new_ptr as *mut u8, diff_size);

            // succeeded!
            Ok(new_ptr)
        } else {
            // roll back
            if old_size > size {
                // this should really rarely happen
                assert!(self.register(ptr.offset(size as isize), old_size - size).is_ok());
            } else {
                assert!(self.forget(ptr.offset(old_size as isize), size - old_size).is_ok());
            }
            
            // failed
            Err(MemoryError::OutOfMemory)
        }
    }
}

#[inline]
pub fn hint() -> usize {
    MEMORY.lock().hint()
}

#[inline]
pub unsafe fn register(ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
    MEMORY.lock().register(ptr, size)
}

#[inline]
pub unsafe fn forget(ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
    MEMORY.lock().forget(ptr, size)
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    MEMORY.lock().allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
    MEMORY.lock().release(ptr, size, align)
}

#[inline]
pub unsafe fn grow(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    MEMORY.lock().grow(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn shrink(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<(), MemoryError> {
    MEMORY.lock().shrink(ptr, old_size, size, align)
}

#[inline]
pub unsafe fn resize(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
    MEMORY.lock().resize(ptr, old_size, size, align)
}

#[inline]
pub fn granularity(size: usize, _: usize) -> usize {
    if size < 32 {
        32
    } else {
        size
    }
}

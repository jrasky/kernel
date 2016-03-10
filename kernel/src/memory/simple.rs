#[cfg(not(test))]
use core::ptr;
#[cfg(not(test))]
use core::mem;
#[cfg(not(test))]
use core::cmp;

#[cfg(test)]
use std::ptr;
#[cfg(test)]
use std::mem;
#[cfg(test)]
use std::cmp;

use spin::Mutex;

use constants::*;
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
    free: *mut Block
}

impl Manager {
    const fn new() -> Manager {
        Manager {
            free: ptr::null_mut()
        }
    }

    unsafe fn register(&mut self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        debug_assert!(!ptr.is_null(), "Tried to register a null block");

        trace!("Registering block at {:#x} of size {:#x}", ptr as usize, size);

        if self.free.is_null() {
            if size < mem::size_of::<Block>() {
                warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                return Err(MemoryError::TinyBlock);
            }
            self.free = ptr as *mut _;
            *self.free.as_mut().unwrap() = Block {
                base: self.free.offset(1) as *mut _,
                end: (self.free as *mut u8).offset(size as isize) as *mut _,
                next: ptr::null_mut(),
                last: ptr::null_mut()
            };

            trace!("{:?}", self.free.as_mut().unwrap().base);
            trace!("{:?}", self.free.as_mut().unwrap().end);
            trace!("{:?}", self.free.as_mut().unwrap().next);
            trace!("{:?}", self.free.as_mut().unwrap().last);

            return Ok(size);
        }

        let base = (ptr as *mut Block).offset(1) as *mut u8;
        let end = (ptr as *mut u8).offset(size as isize) as *mut u8;
        let mut block = self.free.as_mut().unwrap();

        trace!("Registration ends at 0x{:x}", end as u64);

        trace!("{:?}, {:?}, {:?}", base, end, self.free);

        if end < self.free as *mut _ {
            // insert element before the first free element
            trace!("before");
            if size < mem::size_of::<Block>() {
                warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                return Err(MemoryError::TinyBlock);
            }
            self.free = ptr as *mut _;
            *self.free.as_mut().unwrap() = Block {
                base: base,
                end: end,
                next: block,
                last: ptr::null_mut()
            };

            return Ok(size);
        } else if end == self.free as *mut _ {
            // extend the first element backwards
            trace!("a: {:?}", base);
            let new_block = ptr as *mut Block;
            *new_block.as_mut().unwrap() = Block {
                base: base,
                end: block.end,
                next: block.next,
                last: ptr::null_mut()
            };

            if let Some(next) = block.next.as_mut() {
                next.last = new_block;
            }

            self.free = new_block;

            return Ok(size);
        }

        // search in the list for a place to insert this block
        loop {
            trace!("{:?}", block);
            if block.next.is_null() {
                // insert here
                if ptr > block.end {
                    if size < mem::size_of::<Block>() {
                        warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                        return Err(MemoryError::TinyBlock);
                    }
                    block.next = ptr as *mut Block;
                    *block.next.as_mut().unwrap() = Block {
                        base: base,
                        end: end,
                        next: ptr::null_mut(),
                        last: block
                    };

                    return Ok(size);
                } else if ptr == block.end {
                    // extend the last block
                    block.end = end;

                    return Ok(size);
                } else {
                    error!("Unable to register block, likely overlapping");
                    return Err(MemoryError::NoPlace);
                }
            }

            let next = block.next.as_mut().unwrap();

            if ptr > block.end && end < block.next as *mut _ {
                // insert between block and next
                block.next = ptr as *mut Block;
                next.last = ptr as *mut Block;
                if size < mem::size_of::<Block>() {
                    warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                    return Err(MemoryError::TinyBlock);
                }
                *block.next.as_mut().unwrap() = Block {
                    base: base,
                    end: end,
                    next: next,
                    last: block
                };

                return Ok(size);
            } else if ptr == block.end && end == block.next as *mut _ {
                // join the two elements together
                block.end = next.end;
                block.next = next.next;

                return Ok(size);
            } else if ptr == block.end && end < block.next as *mut _ {
                // extend block
                block.end = end;

                return Ok(size);
            } else if ptr > block.end && end == block.next as *mut _ {
                // extend next
                block.next = ptr as *mut Block;
                *block.next.as_mut().unwrap() = Block {
                    base: base,
                    end: next.end,
                    next: next.next,
                    last: block
                };

                return Ok(size);
            } else {
                // advance
                block = block.next.as_mut().unwrap();
            }
        }
    }

    unsafe fn forget(&mut self, ptr: *mut u8, size: usize) -> Result<usize, MemoryError> {
        let mut block = match self.free.as_mut() {
            None => {
                warn!("Tried to forget memory, but nothing was registered");
                return Err(MemoryError::OutOfMemory);
            },
            Some(block) => block
        };

        if size == 0 {
            return Err(MemoryError::TinyBlock);
        }

        trace!("Forgetting at {:#x} size {:#x}", ptr as usize, size);

        let end = (ptr as *mut u8).offset(size as isize) as *mut u8;

        let mut forgotten_size: usize = 0;

        loop {
            trace!("{:?}", block);
            if block.base >= ptr && end >= block.end {
                // block is in the section, remove it
                trace!("Removing block");
                forgotten_size += block.end as usize - block.base as usize;

                if let Some(last) = block.last.as_mut() {
                    last.next = block.next;
                }

                if let Some(next) = block.next.as_mut() {
                    next.last = block.last;
                }
            } else if ptr > block.base && ptr < block.end && end >= block.end {
                // shorten the block
                trace!("shortening block");
                forgotten_size += block.end as usize - ptr as usize;
                block.end = ptr;
            } else if block.base >= ptr && end >= block.base && block.end > end {
                // truncate the front of the block
                trace!("truncating block");
                forgotten_size += end as usize - block.base as usize;

                let new_block = (end as *mut Block).as_mut().unwrap();
                *new_block = Block {
                    base: (end as *mut Block).offset(1) as *mut u8,
                    end: block.end,
                    next: block.next,
                    last: block.last
                };

                if let Some(last) = block.last.as_mut() {
                    last.next = new_block;
                }

                if let Some(next) = block.next.as_mut() {
                    next.last = new_block;
                }
            } else if ptr > block.base && block.end > end {
                // section is entirely in the block, split it in two
                trace!("splitting section");
                let new_block = (end as *mut Block).as_mut().unwrap();
                *new_block = Block {
                    base: (end as *mut Block).offset(1) as *mut u8,
                    end: block.end,
                    next: block.next,
                    last: block as *mut _
                };

                block.end = ptr;

                if let Some(next) = block.next.as_mut() {
                    next.last = new_block;
                }

                block.next = new_block;

                return Ok(size);
            }

            if let Some(next_block) = block.next.as_mut() {
                block = next_block;
            } else {
                // done!
                if forgotten_size > 0 {
                    return Ok(forgotten_size);
                } else {
                    return Err(MemoryError::NoPlace);
                }
            }
        }
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Result<*mut u8, MemoryError> {
        let mut block = self.free;

        let size = granularity(size, align);

        let mut aligned_base;

        trace!("Allocating size 0x{:x} align 0x{:x}", size, align);

        loop {
            if let Some(block_ref) = block.as_mut() {
                aligned_base = constants::align(block as usize, align) as *mut u8;
                let end = (aligned_base as *mut u8).offset(size as isize) as *mut u8;
                if aligned_base < block_ref.end &&
                    block_ref.end as usize - aligned_base as usize >= size
                {
                    // we've found a spot!
                    trace!("Allocating at 0x{:x} to 0x{:x}", aligned_base as u64, end as u64);
                    trace!("{:?}", block_ref);
                    // truncate the block
                    if aligned_base as *mut u8 <= block_ref.base {
                        if ((end as *mut Block).offset(1) as *mut u8) <= block_ref.end {
                            // move the block forward
                            trace!("Moving forward");
                            let new_block = Block {
                                base: (end as *mut Block).offset(1) as *mut u8,
                                end: block_ref.end,
                                next: block_ref.next,
                                last: block_ref.last
                            };
                            
                            if let Some(next) = new_block.next.as_mut() {
                                next.last = end as *mut Block;
                            }

                            if let Some(last) = new_block.last.as_mut() {
                                last.next = end as *mut Block;
                            } else {
                                self.free = end as *mut Block;
                            }

                            *(end as *mut Block).as_mut().unwrap() = new_block;
                        } else {
                            // delete the block
                            trace!("deleting block");

                            if let Some(next) = block_ref.next.as_mut() {
                                next.last = block_ref.last;
                            }

                            if let Some(last) = block_ref.last.as_mut() {
                                last.next = block_ref.next;
                            } else {
                                self.free = block_ref.next;
                            }
                        }
                    } else {
                        // split block in two
                        trace!("Splitting in two");
                        let new_block = (end as *mut Block).as_mut().unwrap();
                        *new_block = Block {
                            base: (end as *mut Block).offset(1) as *mut u8,
                            end: block_ref.end,
                            next: block_ref.next,
                            last: block_ref
                        };

                        if let Some(next) = new_block.next.as_mut() {
                            next.last = new_block;
                        }

                        block_ref.next = new_block;
                        block_ref.end = aligned_base as *mut u8;
                    }

                    // done
                    break;
                } else {
                    // advance
                    block = block_ref.next;
                }
            } else {
                // oom
                return Err(MemoryError::OutOfMemory);
            }
        }

        // produce pointer
        Ok(aligned_base)
    }

    unsafe fn release(&mut self, ptr: *mut u8, size: usize, align: usize) -> Result<usize, MemoryError> {
        let size = granularity(size, align);
        let registered_size = try!(self.register(ptr, size));
        trace!("{}", registered_size);

        if registered_size == size {
            Ok(size)
        } else {
            Err(MemoryError::Overlap)
        }
    }

    unsafe fn grow(&mut self, ptr: *mut u8, old_size: usize, mut size: usize, align: usize) -> Result<(), MemoryError> {
        // adjust size, alignment does not matter in this case
        size = granularity(size, align);

        if size <= old_size {
            // done
            return Ok(());
        }

        let end = (ptr as *mut u8).offset(old_size as isize) as *mut u8;
        let new_end = (ptr as *mut u8).offset(size as isize) as *mut u8;

        trace!("Trying to grow at 0x{:x} from 0x{:x} to 0x{:x}", ptr as u64, end as u64, new_end as u64);

        let mut block = self.free;

        loop {
            trace!("{:?}", block);
            if let Some(block_ref) = block.as_mut() {
                trace!("{:?}", block_ref);
                if block as *mut u8 == end && block_ref.end >= new_end {
                    // we can grow this allocation
                    trace!("grow to {}", size);
                    if block_ref.end > new_end {
                        // shorten the block
                        let new_block = Block {
                            base: (new_end as *mut Block).offset(1) as *mut u8,
                            end: block_ref.end,
                            last: block_ref.last,
                            next: block_ref.next
                        };

                        // update the previous block
                        if let Some(last) = new_block.last.as_mut() {
                            last.next = new_end as *mut Block;
                        } else {
                            // we've moved the first block
                            self.free = new_end as *mut Block;
                        }

                        trace!("{:?}, {:?}", new_end, new_block);
                        *(new_end as *mut Block).as_mut().unwrap() = new_block;
                        return Ok(());
                    } else {
                        // delete the block
                        if let Some(last) = block_ref.last.as_mut() {
                            last.next = block_ref.next;
                        }

                        if let Some(next) = block_ref.next.as_mut() {
                            next.last = block_ref.last;
                        }

                        return Ok(());
                    }
                } else {
                    // advance
                    block = block_ref.next;
                }
            } else {
                // we cannot grow this allocation
                return Err(MemoryError::OutOfSpace);
            }
        }
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
        trace!("Copying");
        ptr::copy(ptr as *mut u8, (&mut store as *mut _ as *mut u8)
                  .offset(diff_size as isize), diff_size);

        if let Err(e) = self.release(ptr, old_size, align) {
            error!("Failed to free pointer on resize: {}", e);
            return Err(e);
        }

        if let Ok(new_ptr) = self.allocate(size, align) {
            trace!("{:?}, {:?}, {:?}", ptr, new_ptr, old_size);

            // copy the data from the old pointer
            ptr::copy(ptr as *mut u8, new_ptr as *mut u8, old_size);

            // some bytes at the beginning might have been clobbered
            // copy data that might have been clobbered
            ptr::copy((&mut store as *mut _ as *mut u8)
                      .offset(diff_size as isize), new_ptr as *mut u8, diff_size);

            // succeeded!
            Ok(new_ptr)
        } else {
            // roll back
            let end = (ptr as *mut u8).offset(old_size as isize) as *mut u8;
            let mut block = self.free;
            loop {
                if let Some(block_ref) = block.as_mut() {
                    if block_ref.base <= ptr && end <= block_ref.end {
                        if ptr > block_ref.base {
                            // truncate the block
                            if end < block_ref.end {
                                // split the block into two
                                let new_block = (end as *mut Block).as_mut().unwrap();
                                new_block.base = end;
                                new_block.end = block_ref.end;
                                new_block.next = block_ref.next;
                                new_block.last = block_ref;
                                block_ref.next = new_block;
                                if let Some(next_block) = new_block.next.as_mut() {
                                    next_block.last = new_block;
                                }
                            }

                            block_ref.end = ptr;

                            // done
                            break;
                        } else {
                            if end < block_ref.end {
                                // truncate the beginning of the block
                                block_ref.base = end;

                                // done
                                break;
                            } else {
                                // allocate the entire block
                                if let Some(last) = block_ref.last.as_mut() {
                                    last.next = block_ref.next;
                                }

                                if let Some(next) = block_ref.next.as_mut() {
                                    next.last = block_ref.last;
                                }

                                // done
                                break;
                            }
                        }
                    } else {
                        block = block_ref.next;
                    }
                } else {
                    // could not find the old block?
                    panic!("Couldn't roll-back resize");
                }
            }

            // failed
            Err(MemoryError::OutOfMemory)
        }
    }
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

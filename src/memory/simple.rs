use super::{Opaque, Header};

use core::ptr;
use core::mem;
use core::cmp;

use spin::Mutex;

use constants::*;
use constants;

static MEMORY: Mutex<Manager> = Mutex::new(Manager {
    free: ptr::null_mut()
});

#[derive(Debug, Clone, Copy)]
struct Block {
    base: *mut Opaque,
    end: *mut Opaque,
    next: *mut Block,
    last: *mut Block,
}

struct Manager {
    free: *mut Block
}

impl Manager {
    unsafe fn register(&mut self, ptr: *mut Opaque, size: usize) -> usize {
        debug_assert!(!ptr.is_null(), "Tried to register a null block");

        debug!("Registering block at {:#x} of size {:#x}", ptr as usize, size);

        if self.free.is_null() {
            if size <= mem::size_of::<Block>() {
                warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                return 0;
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

            return size;
        }

        let base = (ptr as *mut Block).offset(1) as *mut Opaque;
        let end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;
        let mut block = self.free.as_mut().unwrap();

        debug!("Registration ends at 0x{:x}", end as u64);

        trace!("{:?}, {:?}, {:?}", base, end, self.free);

        if end < self.free as *mut _ {
            // insert element before the first free element
            trace!("before");
            if size <= mem::size_of::<Block>() {
                warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                return 0;
            }
            self.free = ptr as *mut _;
            *self.free.as_mut().unwrap() = Block {
                base: base,
                end: end,
                next: block,
                last: ptr::null_mut()
            };

            return size;
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

            return size;
        }

        // search in the list for a place to insert this block
        loop {
            trace!("{:?}", block);
            if block.next.is_null() {
                // insert here
                if ptr > block.end {
                    if size <= mem::size_of::<Block>() {
                        warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                        return 0;
                    }
                    block.next = ptr as *mut Block;
                    *block.next.as_mut().unwrap() = Block {
                        base: base,
                        end: end,
                        next: ptr::null_mut(),
                        last: block
                    };

                    return size;
                } else if ptr == block.end {
                    // extend the last block
                    block.end = end;

                    return size;
                } else {
                    error!("Unable to register block, likely overlapping");
                    return 0;
                }
            }

            let next = block.next.as_mut().unwrap();

            if ptr > block.end && end < block.next as *mut _ {
                // insert between block and next
                block.next = ptr as *mut Block;
                next.last = ptr as *mut Block;
                if size <= mem::size_of::<Block>() {
                    warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                    return 0;
                }
                *block.next.as_mut().unwrap() = Block {
                    base: base,
                    end: end,
                    next: next,
                    last: block
                };

                return size;
            } else if ptr == block.end && end == block.next as *mut _ {
                // join the two elements together
                block.end = next.end;
                block.next = next.next;

                return size;
            } else if ptr == block.end && end < block.next as *mut _ {
                // extend block
                block.end = end;

                return size;
            } else if ptr > block.end && end == block.next as *mut _ {
                // extend next
                block.next = ptr as *mut Block;
                *block.next.as_mut().unwrap() = Block {
                    base: base,
                    end: next.end,
                    next: next.next,
                    last: block
                };

                return size;
            } else {
                // advance
                block = block.next.as_mut().unwrap();
            }
        }
    }

    unsafe fn forget(&mut self, ptr: *mut Opaque, size: usize) -> usize {
        let mut block = match self.free.as_mut() {
            None => {
                warn!("Tried to forget memory, but nothing was registered");
                return 0;
            },
            Some(block) => block
        };

        //trace!("Forgetting at {:#x} size {:#x}", ptr as usize, size);

        let end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;

        let mut forgotten_size: usize = 0;

        loop {
            //trace!("{:?}", block);
            if block.base >= ptr && end >= block.end {
                // block is in the section, remove it
                //trace!("Removing block");
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
                //trace!("truncating block");
                forgotten_size += end as usize - block.base as usize;

                let new_block = (end as *mut Block).as_mut().unwrap();
                *new_block = Block {
                    base: (end as *mut Block).offset(1) as *mut Opaque,
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
                //trace!("splitting section");
                let new_block = (end as *mut Block).as_mut().unwrap();
                *new_block = Block {
                    base: (end as *mut Block).offset(1) as *mut Opaque,
                    end: block.end,
                    next: block.next,
                    last: block as *mut _
                };

                block.end = ptr;

                if let Some(next) = block.next.as_mut() {
                    next.last = new_block;
                }

                block.next = new_block;

                return size;
            }

            if let Some(next_block) = block.next.as_mut() {
                block = next_block;
            } else {
                // done!
                //panic!();
                return forgotten_size;
            }
        }
    }

    unsafe fn allocate(&mut self, mut size: usize, align: usize) -> Option<*mut Opaque> {
        let mut block = self.free;

        size = granularity(size, align);

        let mut aligned_base;
        let mut header_base;

        debug!("Allocating size 0x{:x} align 0x{:x}", size, align);

        loop {
            if let Some(block_ref) = block.as_mut() {
                aligned_base = constants::align(block as usize 
                                                + mem::size_of::<Header>(), align) as *mut Opaque;
                let end = (aligned_base as *mut u8).offset(size as isize) as *mut Opaque;
                header_base = (aligned_base as *mut Header).offset(-1);
                if aligned_base < block_ref.end &&
                    block_ref.end as usize - aligned_base as usize >= size
                {
                    // we've found a spot!
                    debug!("Allocating at 0x{:x} to 0x{:x}", header_base as u64, end as u64);
                    trace!("{:?}", block_ref);
                    trace!("{:?}, {:?}, {:?}", aligned_base, header_base, end);
                    // truncate the block
                    if header_base as *mut Opaque <= block_ref.base {
                        if ((end as *mut Block).offset(1) as *mut Opaque) <= block_ref.end {
                            // move the block forward
                            trace!("Moving forward");
                            let new_block = Block {
                                base: (end as *mut Block).offset(1) as *mut Opaque,
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

                            // this is at least as big as size
                            size = block_ref.end as usize - aligned_base as usize;

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
                            base: (end as *mut Block).offset(1) as *mut Opaque,
                            end: block_ref.end,
                            next: block_ref.next,
                            last: block_ref
                        };

                        if let Some(next) = new_block.next.as_mut() {
                            next.last = new_block;
                        }

                        block_ref.next = new_block;
                        block_ref.end = header_base as *mut Opaque;
                    }

                    // done
                    break;
                } else {
                    // advance
                    block = block_ref.next;
                }
            } else {
                // oom
                return None;
            }
        }

        // set up header
        *header_base.as_mut().unwrap() = Header {
            magic: SIMPLE_MAGIC,
            size: size
        };

        // produce pointer
        Some(aligned_base)
    }

    unsafe fn get_header<'a, 'b>(&'a self, ptr: *mut Opaque) -> Option<&'b mut Header> {
        let header_ptr = (ptr as *mut Header).offset(-1);

        // since we can allocate anywhere, we just check the magic
        if let Some(header) = header_ptr.as_mut() {
            if header.magic != SIMPLE_MAGIC {
                error!("Pointer was not allocated by simple allocator");
                None
            } else {
                Some(header)
            }
        } else {
            error!("Pointer was null");
            None
        }
    }

    unsafe fn release(&mut self, ptr: *mut Opaque) -> Option<usize> {
        if let Some(header) = self.get_header(ptr) {
            let registered_size = self.register(header as *mut _ as *mut _,
                                                header.size + mem::size_of::<Header>());
            trace!("{}", registered_size);
            if registered_size < mem::size_of::<Header>() {
                None
            } else {
                Some(registered_size - mem::size_of::<Header>())
            }
        } else {
            error!("Failed to get allocation header on release");
            None
        }
    }

    unsafe fn grow(&mut self, ptr: *mut Opaque, mut size: usize) -> bool {
        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get allocation header on grow");
                return false;
            },
            Some(header) => header
        };

        // adjust size, alignment does not matter in this case
        size = granularity(size, 0);

        if size <= header.size {
            // done
            return true;
        }

        let end = (ptr as *mut u8).offset(header.size as isize) as *mut Opaque;
        let new_end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;

        debug!("Trying to grow at 0x{:x} from 0x{:x} to 0x{:x}", ptr as u64, end as u64, new_end as u64);

        let mut block = self.free;

        loop {
            trace!("{:?}", block);
            if let Some(block_ref) = block.as_mut() {
                trace!("{:?}", block_ref);
                if block as *mut Opaque == end && block_ref.end >= new_end {
                    // we can grow this allocation
                    trace!("grow to {}", size);
                    if block_ref.end > new_end {
                        // shorten the block
                        let new_block = Block {
                            base: (new_end as *mut Block).offset(1) as *mut Opaque,
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
                        header.size = size;
                        return true;
                    } else {
                        // delete the block
                        if let Some(last) = block_ref.last.as_mut() {
                            last.next = block_ref.next;
                        }

                        if let Some(next) = block_ref.next.as_mut() {
                            next.last = block_ref.last;
                        }

                        header.size = size;
                        return true;
                    }
                } else {
                    // advance
                    block = block_ref.next;
                }
            } else {
                // we cannot grow this allocation
                return false;
            }
        }
    }

    unsafe fn shrink(&mut self, ptr: *mut Opaque, mut size: usize) -> bool {
        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get header on shrink");
                return false;
            },
            Some(header) => {
                header
            }
        };

        // adjust size
        size = granularity(size, 0);
        
        if size >= granularity(header.size, 0) {
            return true;
        }

        let difference = size - header.size;
        let registered_size = self.register((ptr as *mut u8).offset(size as isize) as *mut Opaque, difference);

        trace!("f: {}, {}", difference, registered_size);

        if registered_size < difference {
            return false;
        } else {
            return true;
        }
    }

    unsafe fn resize(&mut self, ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
        trace!("Resizing at {:?} to 0x{:x} with align 0x{:x}", ptr, size, align);
        let header_base = (ptr as *mut Header).offset(-1);
        let header = match self.get_header(ptr) {
            None => {
                error!("Failed to get header on resize");
                return None;
            },
            Some(header) => header
        };

        if (ptr as usize) & (align - 1) == 0 {
            // pointer is already aligned
            trace!("Trying inplace");
            if granularity(size, align) > granularity(header.size, align) {
                trace!("Growing");
                if self.grow(ptr, size) {
                    return Some(ptr);
                }
            } else if granularity(size, align) < granularity(header.size, align) {
                trace!("Shrinking");
                if self.shrink(ptr, size) {
                    return Some(ptr);
                }
            } else {
                // pointer is aligned and the right size, do nothing
                trace!("Doing nothing");
                return Some(ptr);
            }
        }

        // keep data that might be clobbered by release
        let diff_size: usize = cmp::min(mem::size_of::<Block>() - mem::size_of::<Header>(), size);
        let mut store: Block = mem::zeroed();
        trace!("Copying");
        ptr::copy(ptr as *mut u8, (&mut store as *mut _ as *mut u8)
                  .offset(diff_size as isize), diff_size);

        if self.release(ptr).is_none() {
            error!("Failed to free pointer on resize");
            return None;
        }

        if let Some(new_ptr) = self.allocate(size, align) {
            trace!("{:?}, {:?}, {:?}", ptr, new_ptr, header.size);

            // copy the data from the old pointer
            ptr::copy(ptr as *mut u8, new_ptr as *mut u8, header.size);

            // some bytes at the beginning might have been clobbered
            // copy data that might have been clobbered
            ptr::copy((&mut store as *mut _ as *mut u8)
                      .offset(diff_size as isize), new_ptr as *mut u8, diff_size);

            // header might have been clobbered
            header.magic = SIMPLE_MAGIC;
            header.size = size;

            // succeeded!
            Some(new_ptr)
        } else {
            // roll back
            let end = (ptr as *mut u8).offset(header.size as isize) as *mut Opaque;
            let mut block = self.free;
            loop {
                if let Some(block_ref) = block.as_mut() {
                    if block_ref.base <= header_base as *mut Opaque && end <= block_ref.end {
                        if header_base as *mut Opaque > block_ref.base {
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

                            block_ref.end = header_base as *mut Opaque;

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
            None
        }
    }
}

#[inline]
pub unsafe fn register(ptr: *mut Opaque, size: usize) -> usize {
    MEMORY.lock().register(ptr, size)
}

#[inline]
pub unsafe fn forget(ptr: *mut Opaque, size: usize) -> usize {
    MEMORY.lock().forget(ptr, size)
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Option<*mut Opaque> {
    MEMORY.lock().allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut Opaque) -> Option<usize> {
    MEMORY.lock().release(ptr)
}

#[inline]
pub unsafe fn grow(ptr: *mut Opaque, size: usize) -> bool {
    MEMORY.lock().grow(ptr, size)
}

#[inline]
pub unsafe fn shrink(ptr: *mut Opaque, size: usize) -> bool {
    MEMORY.lock().shrink(ptr, size)
}

#[inline]
pub unsafe fn resize(ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
    MEMORY.lock().resize(ptr, size, align)
}

#[inline]
pub fn granularity(size: usize, _: usize) -> usize {
    if size < 32 {
        32
    } else {
        size
    }
}

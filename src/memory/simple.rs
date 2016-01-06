use super::{Opaque, Header};

use core::ptr;
use core::mem;

use spin::Mutex;

use constants::*;
use constants;

static MEMORY: Mutex<Manager> = Mutex::new(Manager {
    free: ptr::null_mut()
});

#[derive(Clone, Copy)]
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

            return size;
        }

        let base = ptr.offset(1);
        let end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;
        let mut block = self.free.as_mut().unwrap();

        if end < block.base {
            // insert element before the first free element
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
        } else if end == block.base {
            // extend the first element backwards
            self.free.as_mut().unwrap().base = base;

            return size;
        }

        // search in the list for a place to insert this block
        loop {
            if block.next.is_null() {
                // insert here
                if base > block.end {
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
                } else if base == block.end {
                    // extend the last block
                    block.end = end;

                    return size;
                } else {
                    error!("Unable to register block, likely overlapping");
                    return 0;
                }
            }

            let next = block.next.as_mut().unwrap();

            if base > block.end && end < next.base {
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
            } else if base == block.end && end == next.base {
                // join the two elements together
                block.end = next.end;
                block.next = next.next;

                return size;
            } else if base == block.end && end < next.base {
                // extend block
                block.end = end;

                return size;
            } else if base > block.end && end == next.base {
                // extend next
                if size <= mem::size_of::<Block>() {
                    warn!("Cannot register a memory block smaller than {}", mem::size_of::<Block>());
                    return 0;
                }

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

        let end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;

        let mut forgotten_size: usize = 0;

        loop {
            if block.base >= end {
                // just advance, we haven't reached our block of interest yet
                if let Some(next_block) = block.next.as_mut() {
                    block = next_block;
                } else {
                    warn!("Tried to forget un unregistered region");
                    return 0;
                }
            } else if block.base <= ptr && end >= block.end {
                // block is in the section, remove it
                if let Some(last) = block.last.as_mut() {
                    last.next = block.next;
                }

                if let Some(next) = block.next.as_mut() {
                    next.last = block.last;
                }

                return size;
            } else if end >= block.end {
                // truncate the beginning of the block
                forgotten_size += end as usize - block.base as usize;
                block.base = end;
            } else if block.base <= ptr {
                // truncate the end of the block
                forgotten_size += block.end as usize - ptr as usize;
                block.end = ptr;
                // and we're done
                return forgotten_size;
            } else {
                unreachable!("Block boundary checks failed");
            }
        }
    }

    unsafe fn allocate(&mut self, size: usize, align: usize) -> Option<*mut Opaque> {
        let mut block = self.free;

        let mut aligned_base;
        let mut header_base;

        loop {
            if let Some(block_ref) = block.as_mut() {
                aligned_base = constants::align(block_ref.base as usize 
                                                + mem::size_of::<Header>(), align) as *mut Opaque;
                let end = (aligned_base as *mut u8).offset(size as isize) as *mut Opaque;
                header_base = (aligned_base as *mut Header).offset(-1);
                if aligned_base < block_ref.end &&
                    block_ref.end as usize - aligned_base as usize >= size
                {
                    // we've found a spot!
                    if header_base as *mut Opaque > block_ref.base {
                        // truncate the block
                        let end = (aligned_base as *mut u8).offset(size as isize) as *mut Opaque;
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

    unsafe fn release(&mut self, ptr: *mut Opaque) -> Option<usize> {
        // pointer and size do not inclue the header
        let header_base = (ptr as *mut Header).offset(-1);
        let header = header_base.as_ref().unwrap();
        let registered_size = self.register(ptr, header.size + mem::size_of::<Header>());
        if registered_size < mem::size_of::<Header>() {
            None
        } else {
            Some(registered_size - mem::size_of::<Header>())
        }
    }

    unsafe fn grow(&mut self, ptr: *mut Opaque, size: usize) -> bool {
        let header_base = (ptr as *mut Header).offset(-1);
        let header = header_base.as_mut().unwrap();
        debug_assert!(size > header.size);
        let end = (ptr as *mut u8).offset(header.size as isize) as *mut Opaque;
        let new_end = (ptr as *mut u8).offset(size as isize) as *mut Opaque;

        let mut block = self.free;

        loop {
            if let Some(block_ref) = block.as_mut() {
                if block_ref.base == end && block_ref.end >= new_end {
                    // we can grow this allocation
                    if block_ref.end > new_end {
                        // shorten the block
                        block_ref.base = new_end;
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
                }
            } else {
                // we cannot grow this allocation
                return false;
            }
        }
    }

    unsafe fn shrink(&mut self, ptr: *mut Opaque, size: usize) -> bool {
        let header_base = (ptr as *mut Header).offset(-1);
        let header = header_base.as_mut().unwrap();
        debug_assert!(size < header.size);
        let difference = size - header.size;
        let registered_size = self.register((ptr as *mut u8).offset(size as isize) as *mut Opaque, difference);
        if registered_size < difference {
            return false;
        } else {
            return true;
        }
    }

    unsafe fn resize(&mut self, ptr: *mut Opaque, size: usize, align: usize) -> Option<*mut Opaque> {
        let header_base = (ptr as *mut Header).offset(-1);
        let header = header_base.as_mut().unwrap();

        if (ptr as usize) | (align - 1) == 0 {
            // pointer is already aligned
            if size > header.size {
                if self.grow(ptr, size) {
                    return Some(ptr);
                }
            } else if size < header.size {
                if self.shrink(ptr, size) {
                    return Some(ptr);
                }
            } else {
                // pointer is aligned and the right size, do nothing
                return Some(ptr);
            }
        }

        if self.release(ptr).is_none() {
            error!("Failed to free pointer on resize");
            return None;
        }

        if let Some(new_ptr) = self.allocate(size, align) {
            ptr::copy(ptr as *mut u8, new_ptr as *mut u8, header.size);
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
pub const fn granularity(size: usize, _: usize) -> usize {
    size
}

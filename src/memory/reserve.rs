use constants::*;

use core::cell::{RefCell, RefMut};

use core::mem;

use super::{Opaque, Header};

static RESERVE: Memory = Memory {
    inner: RefCell::new(MemoryInner {
        slab: [0; RESERVE_SLAB_SIZE],
        map: [0; (RESERVE_SLAB_SIZE + 63) / 64],
    })
};

struct Memory {
    inner: RefCell<MemoryInner>,
}

unsafe impl Sync for Memory {}

struct MemoryInner {
    slab: [u64; RESERVE_SLAB_SIZE],
    // RESERVE_SLAB_SIZE / 8 bytes per block, each byte has 8 bits
    // round up
    map: [u8; (RESERVE_SLAB_SIZE + 63) / 64],
}

impl Memory {
    fn borrow_mut(&self) -> RefMut<MemoryInner> {
        self.inner.borrow_mut()
    }
}

impl MemoryInner {
    unsafe fn allocate(&mut self, size: usize, align: usize) -> Option<*mut Opaque> {
        // round up to the number of blocks we need to allocate
        let blocks = (size + mem::size_of::<Header>() + 7) / 8;

        let mut start: usize = 0;
        let mut acc: usize = 0;
        let mut pos: usize = 0;
        let mut subpos: usize = 0;
        loop {
            // check for oom
            if pos >= RESERVE_SLAB_SIZE {
                error!("Failed to allocate reserve memory");
                return None;
            }

            // check for free blocks
            if self.map[pos] & (1 << subpos) == 0 {
                // block is free
                if acc > 0 {
                    acc += 1;
                } else if align == 0 || ((self as *const _ as usize) // offset of the slab
                                         + mem::size_of::<Header>() // offset for the header
                                         + (((pos * 8) + subpos) * 8)) // offset into slab
                // aligned to given number of bytes
                    & (align - 1) == 0 {
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
                let header = &mut self.slab[start] as *mut _ as *mut Header;
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

    unsafe fn release(&mut self, ptr: *mut Opaque) -> Option<usize> {
        let header_ptr = (ptr as *mut Header).offset(-1);
        
        if header_ptr < (self as *mut _ as *mut _) {
            error!("Tried to free a pointer not in the reserve slab");
            return None;
        }

        let start = (header_ptr as usize) - (self as *mut _ as usize);

        if start > RESERVE_SLAB_SIZE {
            error!("Tried to free a pointer not in the reserve slab");
            return None;
        }

        let header = match header_ptr.as_mut() {
            None => {
                error!("Tried to free a null pointer");
                return None;
            },
            Some(header) => header
        };

        if header.magic != RESERVE_MAGIC {
            error!("Tried to free an invalid pointer");
            return None;
        }

        let blocks = (header.size + mem::size_of::<Header>() + 63)  / 64;
        
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

        let size = header.size;

        // reset header info
        header.magic = 0;
        header.size = 0;

        // successfully freed memory
        Some(size)
    }
}

#[inline]
pub unsafe fn allocate(size: usize, align: usize) -> Option<*mut Opaque> {
    RESERVE.borrow_mut().allocate(size, align)
}

#[inline]
pub unsafe fn release(ptr: *mut Opaque) -> Option<usize> {
    RESERVE.borrow_mut().release(ptr)
}

#[inline]
pub fn granularity(size: usize, _: usize) -> usize {
    ((size + mem::size_of::<Header>() + 7) / 8) * 8
}

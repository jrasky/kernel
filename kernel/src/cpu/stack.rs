use alloc::heap;

#[cfg(not(test))]
use core::ptr::Unique;
#[cfg(test)]
use std::ptr::Unique;

use memory::Opaque;

use constants::*;

extern "C" {
    static _long_stack: u8;
}

pub struct Stack {
    buffer: Unique<u8>,
    size: usize,
    drop: bool,
}

impl Drop for Stack {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                heap::deallocate(self.buffer.get_mut(), self.size, 16);
            }
        }
    }
}

impl Stack {
    pub fn create(size: usize) -> Stack {
        Stack {
            buffer: unsafe { Unique::new(heap::allocate(size, 16)) },
            size: size,
            drop: true,
        }
    }

    pub unsafe fn kernel() -> Stack {
        Stack {
            buffer: Unique::new(&_long_stack as *const _ as *mut _),
            size: STACK_SIZE,
            drop: false,
        }
    }

    pub fn get_ptr(&self) -> *mut Opaque {
        trace!("stack {:p} size {:x}", self.buffer, self.size);
        unsafe { (self.buffer.get() as *const u8 as *mut u8).offset(self.size as isize) as *mut _ }
    }
}

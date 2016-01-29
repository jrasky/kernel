use alloc::heap;

use memory::Opaque;

use constants::*;

extern "C" {
    static _stack_top: u8;
}

pub struct Stack {
    buffer: *mut u8,
    size: usize,
    drop: bool,
}

impl Drop for Stack {
    fn drop(&mut self) {
        if self.drop {
            unsafe {
                heap::deallocate(self.buffer, self.size, 16);
            }
        }
    }
}

impl Stack {
    pub fn create(size: usize) -> Stack {
        Stack {
            buffer: unsafe { heap::allocate(size, 16) },
            size: size,
            drop: true,
        }
    }

    pub unsafe fn kernel() -> Stack {
        Stack {
            buffer: &_stack_top as *const _ as *mut _,
            size: STACK_SIZE,
            drop: false,
        }
    }

    pub fn get_ptr(&self) -> *mut Opaque {
        trace!("stack {:?} size {:x}", self.buffer, self.size);
        unsafe { self.buffer.offset(self.size as isize) as *mut _ }
    }
}

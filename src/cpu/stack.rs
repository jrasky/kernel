use alloc::heap;

use memory::Opaque;

pub struct Stack {
    buffer: *mut u8,
    size: usize
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            heap::deallocate(self.buffer, self.size, 16);
        }
    }
}

impl Stack {
    pub fn create(size: usize) -> Stack {
        Stack {
            buffer: unsafe {heap::allocate(size, 16)},
            size: size
        }
    }

    pub fn get_ptr(&self) -> *mut Opaque {
        trace!("stack {:?} size {:x}", self.buffer, self.size);
        unsafe {self.buffer.offset(self.size as isize) as *mut _}
    }
}

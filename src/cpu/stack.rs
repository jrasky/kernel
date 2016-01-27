use alloc::raw_vec::RawVec;

use memory::Opaque;

pub struct Stack {
    buffer: RawVec<u8>
}

impl Stack {
    pub fn create(size: usize) -> Stack {
        Stack {
            buffer: RawVec::with_capacity(size)
        }
    }

    pub fn get_ptr(&self) -> *mut Opaque {
        let cap = self.buffer.cap();
        trace!("stack {:?} size {:x}", self.buffer.ptr(), cap);
        unsafe {self.buffer.ptr().offset(cap as isize) as *mut _}
    }
}

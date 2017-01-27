use std::fmt::Debug;

use std::ptr;
use std::fmt;

use alloc::raw_vec::RawVec;

use alloc::heap;

pub struct Stack {
    buffer: Option<RawVec<u8>>
}

impl Debug for Stack {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref buffer) = self.buffer {
            write!(fmt, "Stack {{ buffer: Some {{ end: 0x{:x} size: 0x{:x} }} }}",
                   self.get_ptr() as usize, buffer.cap())
        } else {
            write!(fmt, "Stack {{ buffer: None }}")
        }
    }
}

impl Stack {
    pub fn new(size: usize) -> Stack {
        Stack {
            buffer: Some(unsafe { RawVec::from_raw_parts(heap::allocate(size, 16), size) }),
        }
    }

    pub fn dummy() -> Stack {
        Stack {
            buffer: None
        }
    }

    pub fn get_ptr(&self) -> *mut u8 {
        if let Some(ref buffer) = self.buffer {
            let size = buffer.cap();
            unsafe { buffer.ptr().offset(size as isize) }
        } else {
            ptr::null_mut()
        }
    }
}

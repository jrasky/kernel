use include::*;

use cpu::stack::Stack;

pub struct Descriptor {
    base: u64,
    size: u32,
    privilege_level: u8,
    busy: bool
}

pub struct Segment {
    buffer: RawVec<u8>,
    privilege_level: u8,
    io_offset: u16,
    interrupt_stack_table: [Option<Stack>; 7],
    stack_pointers: [Option<Stack>; 3]
}

impl Descriptor {
    pub fn as_entry(&self) -> [u64; 2] {
        trace!("a1: {:?}", self.size);
        trace!("a2: {:?}", self.base);
        let mut lower =
            ((self.base & (0xff << 24)) << 32) | ((self.base & 0xffffff) << 16) // base address
            | (self.size & (0xf << 16)) as u64 | (self.size - 1 & 0xffff) as u64 // limit
            | (1 << 47) | (1 << 43) | (1 << 40); // present, 64-bit, TSS

        assert!(self.privilege_level < 4);

        lower |= (self.privilege_level as u64) << 45; // privilege level
        
        if self.busy {
            lower |= 1 << 41; // busy
        }

        trace!("{:x}, {:x}", self.base >> 32, lower);

        [lower, self.base >> 32]
    }
}

impl Segment {
    pub fn new(interrupt_stack_table: [Option<Stack>; 7],
               stack_pointers: [Option<Stack>; 3], privilege_level: u8) -> Segment {
        Segment {
            buffer: RawVec::with_capacity(0x68),
            privilege_level: privilege_level,
            io_offset: 0, // don't handle this right now
            interrupt_stack_table: interrupt_stack_table,
            stack_pointers: stack_pointers
        }
    }

    pub fn get_register(&self) -> Descriptor {
        Descriptor {
            base: self.buffer.ptr() as u64,
            size: 0x68,
            privilege_level: self.privilege_level,
            busy: false
        }
    }

    pub unsafe fn save(&mut self) -> *mut u8 {
        // make sure our buffer is big enough
        // don't handle i/o map right now
        self.buffer.reserve(0, 0x68);

        // copy the data to our buffer
        let ptr = self.buffer.ptr();
        trace!("b: {:?}", ptr);
        self.copy_to(ptr);

        // produce the buffer
        ptr
    }

    unsafe fn copy_to(&self, mut tss: *mut u8) {
        // reserved
        ptr::copy([0u8; 4].as_ptr(), tss, 4);
        tss = tss.offset(4);

        // stack pointers
        let ptrs: Vec<u64> = self.stack_pointers.iter().map(|stack| {
            if let &Some(ref stack) = stack {
                stack.get_ptr() as u64
            } else {
                0
            }
        }).collect();
        ptr::copy(ptrs.as_ptr(), tss as *mut u64, 3);
        tss = tss.offset(U64_BYTES as isize * 3);

        // reserved
        ptr::copy([0u8; 8].as_ptr(), tss, 8);
        tss = tss.offset(8);

        // interrupt stack table
        let ptrs: Vec<u64> = self.interrupt_stack_table.iter().map(|stack| {
            if let &Some(ref stack) = stack {
                stack.get_ptr() as u64
            } else {
                0
            }
        }).collect();
        ptr::copy(ptrs.as_ptr(), tss as *mut u64, 7);
        tss = tss.offset(U64_BYTES as isize * 7);

        // reserved
        ptr::copy([0u8; 10].as_ptr(), tss, 10);
        tss = tss.offset(10);

        // i/o map offset
        *(tss as *mut u16).as_mut().unwrap() = self.io_offset;
        // done
    }
}

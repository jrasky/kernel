use std::fmt::{Debug, Formatter};

use alloc::raw_vec::RawVec;

use std::fmt;
use std::str;
use std::ptr;

use collections::Vec;

#[repr(packed)]
struct Register {
    size: u16,
    base: u64
}

pub struct Table {
    buffer: RawVec<u8>,
    tss: Vec<super::tss::Segment>
}

impl Debug for Register {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "Register {{ size: 0x{:x}, base: 0x{:x} }}", self.size, self.base)
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        panic!("Tried to drop GDT");
    }
}

impl Table {
    pub fn new(tss: Vec<super::tss::Segment>) -> Table {
        Table {
            buffer: RawVec::new(),
            tss: tss
        }
    }

    pub unsafe fn set_task(&mut self, task_index: u16) {
        assert!((task_index as usize) < self.tss.len());

        // task selector has to be indirected through a memory location
        #[cfg(not(test))]
        asm!("ltr $0"
             :: "r"((task_index + 3) << 3)
             :: "volatile", "intel"); // could modify self
    }

    pub unsafe fn install(&mut self) {
        // write out to buffer
        let res = self.save();

        debug!("{:?}", res);

        // load the new global descriptor table,
        // and reload the segments
        #[cfg(not(test))]
        asm!("lgdt $0;"
             :: "*m"(&res)
             :: "intel");

        // only reload segments if we're already in long mode
        #[cfg(not(test))]
        #[cfg(target_pointer_width = "64")]
        asm!(concat!(
            "push 0x08;",
            "lea rax, .target;",
            "push rax;", // there is no push 64 bit immediate
            "retfq;",
            ".target:",
            "mov ax, 0x10;",
            "mov ds, ax;",
            "mov es, ax;",
            "mov fs, ax;",
            "mov gs, ax;",
            "mov ss, ax;") ::: "rax" : "intel", "volatile");

        // no change in code or data selector from setup in bootstrap
        // so no far jump to reload selector
    }

    unsafe fn save(&mut self) -> Register {
        // make sure we have enough space
        let len = self.tss.len();
        self.buffer.reserve(0, 24 + 16 * len);

        // copy data
        let ptr = self.buffer.ptr();
        let gdtr = self.copy_to(ptr);

        // save TSS list
        for tss in self.tss.iter_mut() {
            tss.save();
        }

        // return pointer to GDT register
        gdtr
    }

    unsafe fn copy_to(&self, gdt: *mut u8) -> Register {
        // u64 is a more convenient addressing mode
        let top = gdt as u64;
        let mut gdt = gdt as *mut u64;

        // first three entries are static, must match those set before jump to
        // long mode, and requirements for syscall instruction
        let header: [u64; 3] = [
            0, // null
            0xffff | (0x9 << 44) | (0xbf << 48) | (0xa << 40), // code
            0xffff | (0x9 << 44) | (0xdf << 48) | (0x2 << 40) // data
        ];

        trace!("{:?}", gdt);
        trace!("{:?}", header);
        trace!("{:?}", header.as_ptr());

        ptr::copy(header.as_ptr(), gdt, 3);

        gdt = gdt.offset(3);

        // copy TSS descriptors

        for desc in self.tss.iter() {
            ptr::copy(desc.get_register().as_entry().as_ptr(), gdt, 2);
            gdt = gdt.offset(2);
        }

        Register {
            size: (gdt as u64 - top - 1) as u16,
            base: top
        }
    }
}

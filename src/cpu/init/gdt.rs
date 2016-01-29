use collections::Vec;

use alloc::raw_vec::RawVec;

use core::ptr;

#[repr(packed)]
#[derive(Debug)]
struct Register {
    size: u16,
    base: u64
}

pub struct Table {
    buffer: RawVec<u8>,
    tss: Vec<super::tss::Segment>
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
        asm!("ltr $0"
             :: "r"((task_index + 3) << 3)
             :: "volatile", "intel"); // could modify self
    }

    pub unsafe fn install(&mut self) {
        // lgdt needs a compile-time constant location
        // we basically have to use a mutable static for this
        static mut REGISTER: Register = Register {
            size: 0,
            base: 0
        };

        // write out to buffer
        let res = self.save();

        // save info in static
        REGISTER.size = res.size;
        REGISTER.base = res.base;

        debug!("{:?}", REGISTER);

        // load the new global descriptor table,
        // and reload the segments
        asm!(concat!(
            "lgdt $0;",
            "call _reload_segments;")
             :: "i"(&REGISTER)
             : "{ax}"
             : "intel");

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

        // first three entries are static
        let header: [u64; 3] = [
            0, // null
            (1 << 44) | (1 << 47) | (1 << 41) | (1 << 43) | (1 << 53), // code
            (1 << 44) | (1 << 47) | (1 << 41)]; // data

        trace!("{:?}", gdt);
        trace!("{:?}", header);
        trace!("{:?}", header.as_ptr());

        ptr::copy(header.as_ptr(), gdt, 3);

        gdt = gdt.offset(3);

        // copy TSS descriptors

        for desc in self.tss.iter() {
            desc.copy_register(gdt);
            gdt = gdt.offset(2);
        }

        Register {
            size: (gdt as u64 - top - 1) as u16,
            base: top
        }
    }
}

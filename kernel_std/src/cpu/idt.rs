use include::*;

#[repr(packed)]
#[derive(Debug)]
struct Register {
    size: u16,
    base: u64,
}

#[derive(Debug)]
pub struct Descriptor {
    target: unsafe extern "C" fn (),
    segment: u16,
    present: bool,
    stack: u8,
}

pub struct Table {
    buffer: RawVec<u8>,
    descriptors: Vec<Descriptor>,
}

unsafe extern "C" fn _dummy_target() {
    unreachable!("dummy interrupt descriptor reached");
}

impl Drop for Table {
    fn drop(&mut self) {
        panic!("Tried to drop IDT");
    }
}

impl Descriptor {
    pub const fn placeholder() -> Descriptor {
        Descriptor {
            target: _dummy_target,
            segment: 0,
            present: false,
            stack: 0,
        }
    }

    pub const fn new(target: unsafe extern "C" fn (), stack: u8) -> Descriptor {
        Descriptor {
            target: target,
            segment: 1 << 3, // second segment, GDT, RPL 0
            present: true,
            stack: stack,
        }
    }

    fn as_entry(&self) -> [u64; 2] {
        if !self.present {
            // make everything zero in this case
            return [0, 0];
        }

        // we only use interrupt gates, not trap gates
        // this is because it's not very rustic to be reentrant, so avoid it
        // if possible

        let lower = ((self.target as u64 & (0xffff << 16)) << 32) | (self.target as u64 & 0xffff) |
                    ((self.segment as u64) << 16) |
                    ((self.stack as u64) << 32) | (1 << 47) | (0x0e << 40); // present, interrupt gate

        trace!("0x{:x}, 0x{:x}", lower, self.target as u64 >> 32);

        [lower, self.target as u64 >> 32]
    }
}

impl Table {
    pub fn new(descriptors: Vec<Descriptor>) -> Table {
        Table {
            buffer: RawVec::new(),
            descriptors: descriptors,
        }
    }

    pub unsafe fn install(&mut self) {
        static mut REGISTER: Register = Register { size: 0, base: 0 };

        // write out IDT
        let res = self.save();

        if res.size == 0 {
            // do nothing
            return;
        }

        // save info in register
        REGISTER.size = res.size;
        REGISTER.base = res.base;

        trace!("aoe: {:?}", REGISTER);

        #[cfg(not(test))]
        asm!("lidt $0"
             :: "i"(&REGISTER)
             :: "intel");
    }

    pub unsafe fn early_install(descriptors: &[Descriptor], mut idt: *mut u64) {
        let len = descriptors.len();
        let top = idt as u64;

        if len == 0 {
            // do nothing
            return;
        }

        // copy data
        for desc in descriptors.iter() {
            ptr::copy(desc.as_entry().as_ptr(), idt, 2);
            idt = idt.offset(2);
        }

        static mut REGISTER: Register = Register { size: 0, base: 0 };

        // save register info
        REGISTER.size = (idt as u64 - top - 1) as u16;
        REGISTER.base = top;

        // install IDT
        #[cfg(not(test))]
        asm!("lidt $0"
             :: "i"(&REGISTER)
             :: "intel");
    }

    unsafe fn save(&mut self) -> Register {
        let len = self.descriptors.len();

        if len == 0 {
            // do nothing if we have no descriptors
            return Register {
                size: 0,
                base: self.buffer.ptr() as u64,
            };
        }

        self.buffer.reserve(0, 36 * len);

        // copy data
        let ptr = self.buffer.ptr();
        let idtr = self.copy_to(ptr);

        // produce idt register
        idtr
    }

    unsafe fn copy_to(&self, idt: *mut u8) -> Register {
        let top = idt as u64;
        let mut idt = idt as *mut u64;

        for desc in self.descriptors.iter() {
            trace!("{:?}", desc);
            ptr::copy(desc.as_entry().as_ptr(), idt, 2);
            idt = idt.offset(2);
        }

        Register {
            size: (idt as u64 - top - 1) as u16,
            base: top,
        }
    }
}

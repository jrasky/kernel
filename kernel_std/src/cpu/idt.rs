use std::ptr::Shared;

use alloc::boxed::Box;

use std::str;
use std::ptr;

use collections::btree_map::BTreeMap;

#[repr(packed)]
#[derive(Debug)]
struct Register {
    size: u16,
    base: usize,
}

#[derive(Debug)]
pub struct Descriptor {
    target: u64,
    segment: u16,
    present: bool,
    stack: u8,
}

pub struct Table {
    descriptors: BTreeMap<u8, Descriptor>,
    table: Shared<u64>
}

pub unsafe extern "C" fn _dummy_target() {
    unreachable!("dummy interrupt descriptor reached");
}

impl Drop for Table {
    fn drop(&mut self) {
        if !self.table.is_null() {
            panic!("Tried to drop IDT");
        }
    }
}

impl Descriptor {
    pub fn placeholder() -> Descriptor {
        Descriptor {
            target: _dummy_target as u64,
            segment: 0,
            present: false,
            stack: 0,
        }
    }

    pub const fn new(target: u64, stack: u8) -> Descriptor {
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

        let lower = ((self.target & (0xffff << 16)) << 32) | (self.target & 0xffff) |
        ((self.segment as u64) << 16) |
        ((self.stack as u64) << 32) | (1 << 47) | (0x0e << 40); // present, interrupt gate

        trace!("0x{:x}, 0x{:x}", lower, self.target >> 32);

        [lower, self.target >> 32]
    }
}

impl Table {
    pub fn new() -> Table {
        Table {
            descriptors: BTreeMap::new(),
            table: unsafe { Shared::new(ptr::null_mut()) }
        }
    }

    pub fn insert(&mut self, vector: u8, descriptor: Descriptor) -> Option<Descriptor> {
        self.descriptors.insert(vector, descriptor)
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
             :: "m"(&REGISTER)
             :: "intel", "volatile");
    }

    unsafe fn save(&mut self) -> Register {
        if self.descriptors.is_empty() {
            // do nothing if we have no descriptors
            return Register {
                size: 0,
                base: 0
            };
        }

        // There should be a max if it isn't empty
        let len = *self.descriptors.keys().max().unwrap() as usize;

        // initialize everything to zero
        let mut buffer = vec![0; len * 2];

        // copy data
        for (vector, desc) in self.descriptors.iter() {
            let entry = desc.as_entry();

            buffer[*vector as usize * 2] = entry[0];
            buffer[*vector as usize * 2 + 1] = entry[1];
        }

        // get a raw pointer
        let ptr = Shared::new(Box::into_raw(buffer.into_boxed_slice()) as *mut u64);

        self.table = ptr;

        // produce idt register
        Register {
            size: len as u16 * 2 - 1,
            base: *ptr as *mut u64 as usize
        }
    }
}

pub unsafe fn early_install(descriptors: &[Descriptor], mut idt: *mut u64) {
    let len = descriptors.len();
    let top = idt as usize;

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
    REGISTER.size = (idt as usize - top - 1) as u16;
    REGISTER.base = top;

    // install IDT
    #[cfg(not(test))]
    asm!("lidt $0"
         :: "m"(&REGISTER)
         :: "intel", "volatile");
}

pub mod gdt {
    use collections::{Vec, String};

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

        pub unsafe fn save(&mut self) -> Register {
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
                ptr::copy(desc.get_register().as_entry().as_ptr(), gdt, 2);
                gdt = gdt.offset(2);
            }

            Register {
                size: (gdt as u64 - top - 1) as u16,
                base: top
            }
        }
    }

}

pub mod idt {
    use collections::{Vec, String};

    use core::ptr;

    use alloc::raw_vec::RawVec;

    #[repr(packed)]
    #[derive(Debug)]
    struct Register {
        size: u16,
        base: u64
    }

    #[derive(Debug)]
    pub struct Descriptor {
        target: u64,
        segment: u16,
        present: bool,
        stack: u8
    }

    pub struct Table {
        buffer: RawVec<u8>,
        descriptors: Vec<Descriptor>
    }

    impl Descriptor {
        pub const fn placeholder() -> Descriptor {
            Descriptor {
                target: 0,
                segment: 0,
                present: false,
                stack: 0
            }
        }

        pub const fn new(target: u64, stack: u8) -> Descriptor {
            Descriptor {
                target: target,
                segment: 1 << 3, // second segment, GDT, RPL 0
                present: true,
                stack: stack
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

            let mut lower = ((self.target & (0xffff << 16)) << 32) | (self.target & 0xffff) // base address
                | ((self.segment as u64) << 16) // Segment Selector
                | ((self.stack as u64) << 32) // IST selector
                | (1 << 47) | (0x0e << 40); // present, interrupt gate

            trace!("{:?}, {:?}", lower, self.target >> 32);

            [lower, self.target >> 32]
        }
    }

    impl Table {
        pub fn new(descriptors: Vec<Descriptor>) -> Table {
            Table {
                buffer: RawVec::new(),
                descriptors: descriptors
            }
        }

        pub unsafe fn install(&mut self) {
            static mut REGISTER: Register = Register {
                size: 0,
                base: 0
            };

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

            asm!("lidt $0"
                 :: "i"(&REGISTER)
                 :: "intel");
        }

        pub unsafe fn save(&mut self) -> Register {
            let len = self.descriptors.len();

            if len == 0 {
                // do nothing if we have no descriptors
                return Register {
                    size: 0,
                    base: self.buffer.ptr() as u64
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
                base: top
            }
        }
    }

}

pub mod tss {
    use collections::{Vec, String};

    use core::ptr;

    use alloc::raw_vec::RawVec;

    use cpu::stack::Stack;

    struct Descriptor {
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
            tss = tss.offset(::core::u64::BYTES as isize * 3);

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
            tss = tss.offset(::core::u64::BYTES as isize * 7);

            // reserved
            ptr::copy([0u8; 10].as_ptr(), tss, 10);
            tss = tss.offset(10);

            // i/o map offset
            *(tss as *mut u16).as_mut().unwrap() = self.io_offset;
            // done
        }
    }

}

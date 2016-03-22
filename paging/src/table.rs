use include::*;

use frame::FrameSize;

use memory;

struct Table {
    basis: *mut u64,
    size: FrameSize
}

impl Drop for Table {
    for idx in 0..0x200 {
        let entry = unsafe {ptr::read(self.basis.offset(idx))};

        if entry & 0x1 == 0 && entry & 0x80 == 0 {
            if let Some(size) = size.get_next() {
                mem::drop(Table {
                    basis: canonicalize(entry & PAGE_ADDR_MASK) as *mut _,
                    size: size
                });

                continue;
            }
        }

        // otherwise free the buffer
        unsafe {
            // ignore result
            let _ = memory::release(canonicalize(entry & PAGE_ADDR_MASK) as *mut _,
                                    0x200 * U64_BYTES, 0x1000);
        }
    }
}

impl Table {
    pub const fn new(basis: *mut u64) -> Table {
        Table {
            basis: basis,
            size: FrameSize::Giant
        }
    }
}

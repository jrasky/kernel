pub use c::boot_c_panic;

use std::ptr;

use alloc::heap;

use constants::*;

use kernel_std::BootInfo;

mod c {
    use std::ptr;
    use std::slice;
    use std::str;

    #[repr(C)]
    pub struct memory_map_entry {
        addr: u64,
        len: u64,
        ty: u32,
        reserved: u32
    }

    #[repr(C)]
    pub struct boot_info {
        command_line_size: usize,
        command_line: *const u8,
        memory_map_size: usize,
        memory_map: *const memory_map_entry
    }

    extern "C" {
        fn parse_multiboot_info(info: *const u32) -> *const boot_info;
    }

    #[inline(never)]
    #[cold]
    #[no_mangle]
    pub extern "C" fn boot_c_panic(message: *const u8) -> ! {
        let mut size: isize = 0;

        while ptr::read(message.offset(size)) != 0 {
            size += 1;
        }

        let str_slice = slice::from_raw_parts(message, size as usize);

        match str::from_utf8(str_slice) {
            Ok(s) => {
                panic!("{}", s);
            }
            Err(e) => {
                panic!("C called panic with invalid string: {}", e);
            }
        }
    }
}

pub unsafe fn create_boot_info(multiboot_info: *const u32) -> BootInfo {
    let info = c::parse_multiboot_info(multiboot_info);

    let command_line;

    if info.command_line as usize + info.command_line_size > OPTIMISTIC_HEAP {
        command_line = heap::allocate(info.command_line_size, 1);
        ptr::copy(info.command_line, command_line, info.command_line_size);
    } else {
        command_line = info.command_line;
    }

    // same for the memory map
    let memory_map;

    if info.memory_map as usize + info.memory_map_size > OPTIMIST_HEAP {
        memory_map = heap::allocate(info.memory_map_size, U64_BYTES);
        ptr::copy(info.memory_map, memory_map, info.memory_map_size);
    } else {
        memory_map = info.memory_map;
    }

    BootInfo {
        command_line_size: info.command_line_size as u64,
        command_line: command_line as u64,
        memory_map_size: info.memory_map_size as u64,
        memory_map: memory_map as u64
    }
}


pub use c::boot_c_panic;

use std::ptr;
use std::mem;

use alloc::heap;

use constants::*;

use kernel_std::BootInfo;

mod c {
    use std::ptr;
    use std::slice;
    use std::str;

    #[repr(C)]
    pub struct boot_info {
        command_line_size: usize,
        command_line: *const u8,
        memory_map_size: usize,
        memory_map: *const (),
        modules: *const module
    }

    #[repr(C)]
    pub struct module {
        start: u64,
        len: u64,
        cmdline: *const u8
    }

    extern "C" {
        fn parse_multiboot_info(info: *const ()) -> *const boot_info;
        fn find_heap(mmap: *const ()) -> u64;
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

pub struct BootCInfo {
    command_line_size: usize,
    command_line: *const u8,
    memory_map_size: usize,
    memory_map: *const (),
    initial_heap: u64
}

pub unsafe fn create_boot_info(multiboot_info: *const ()) -> BootCInfo {
    let info = c::parse_multiboot_info(multiboot_info);

    if info.memory_map.is_null() {
        panic!("Did not get memory map in boot info");
    }

    let heap = c::find_heap(info.memory_map);

    if heap == 0 {
        panic!("Could not place initial heap");
    }

    let command_line;

    if info.command_line.is_null() {
        command_line = ptr::null();
    } else if info.command_line as usize + info.command_line_size > OPTIMISTIC_HEAP {
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

    // free boot_info struct
    heap::deallocate(info, mem::size_of::<c::boot_info>(), mem::size_of::<usize>());

    BootCInfo {
        command_line_size: info.command_line_size,
        command_line: command_line,
        memory_map_size: info.memory_map_size,
        memory_map: memory_map,
        initial_heap: heap
    }
}


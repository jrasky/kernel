#![feature(lang_items)]
#![feature(ptr_as_ref)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(reflect_marker)]
#![feature(alloc)]
#![feature(collections)]
#![feature(unwind_attributes)]
#![feature(stmt_expr_attributes)]
#![feature(asm)]
#![feature(num_bits_bytes)]
#![no_std]
extern crate rlibc;
extern crate spin;
extern crate alloc;
#[macro_use]
extern crate collections;
extern crate elfloader;

use collections::{Vec, String};

use elfloader::elf;

use core::fmt;
use core::slice;
use core::str;
use core::ptr;

use alloc::raw_vec::RawVec;

use constants::*;

#[macro_use]
mod log;
mod error;
mod memory;
mod constants;

// pub use since they're exported
pub use memory::{__rust_allocate,
                 __rust_deallocate,
                 __rust_reallocate,
                 __rust_reallocate_inplace,
                 __rust_usable_size};

extern "C" {
    static _image_begin: u8;
    static _image_end: u8;
}

struct Stack {
    buffer: RawVec<u8>
}

#[repr(packed)]
#[derive(Debug)]
struct GDTRegister {
    size: u16,
    base: u64
}

#[repr(packed)]
#[derive(Debug)]
struct IDTRegister {
    size: u16,
    base: u64
}

struct TaskStateSegment {
    buffer: RawVec<u8>,
    privilege_level: u8,
    io_offset: u16,
    interrupt_stack_table: [Option<Stack>; 7],
    stack_pointers: [Option<Stack>; 3]
}

struct TSSDescriptor {
    base: u64,
    size: u32,
    privilege_level: u8,
    busy: bool
}

#[derive(Debug)]
struct IDTDescriptor {
    target: u64,
    segment: u16,
    interrupt: bool,
    present: bool,
    stack: u8
}

struct GlobalDescriptorTable {
    buffer: RawVec<u8>,
    tss: Vec<TaskStateSegment>
}

struct InterruptDescriptorTable {
    buffer: RawVec<u8>,
    descriptors: Vec<IDTDescriptor>
}

struct MBInfoMemTag {
    base_addr: u64,
    length: u64,
    addr_type: u32,
}

#[derive(Debug)]
struct MBElfSymTag {
    ty: u32,
    size: u32,
    num: u32,
    entsize: u32,
    shndx: u32
}

impl Stack {
    fn create(size: usize) -> Stack {
        Stack {
            buffer: RawVec::with_capacity(size)
        }
    }

    fn get(&self) -> *mut memory::Opaque {
        let cap = self.buffer.cap();
        trace!("stack {:?} size {:x}", self.buffer.ptr(), cap);
        unsafe {self.buffer.ptr().offset(cap as isize) as *mut _}
    }
}

impl TSSDescriptor {
    fn as_entry(&self) -> [u64; 2] {
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

impl IDTDescriptor {
    const fn placeholder() -> IDTDescriptor {
        IDTDescriptor {
            target: 0,
            segment: 0,
            interrupt: false,
            present: false,
            stack: 0
        }
    }

    const fn new(target: u64, interrupt: bool, stack: u8) -> IDTDescriptor {
        IDTDescriptor {
            target: target,
            segment: 1 << 3, // second segment, GDT, RPL 0
            interrupt: interrupt,
            present: true,
            stack: stack
        }
    }

    fn as_entry(&self) -> [u64; 2] {
        if !self.present {
            // make everything zero in this case
            return [0, 0];
        }

        let mut lower = ((self.target & (0xffff << 16)) << 32) | (self.target & 0xffff) // base address
            | ((self.segment as u64) << 16) // Segment Selector
            | ((self.stack as u64) << 32) // IST selector
            | (1 << 47); // present

        if self.interrupt {
            // interrupt gate
            lower |= 0x0e << 40;
        } else {
            // trap gate
            lower |= 0x0f << 40;
        }

        trace!("{:?}, {:?}", lower, self.target >> 32);

        [lower, self.target >> 32]
    }
}

impl TaskStateSegment {
    fn new(interrupt_stack_table: [Option<Stack>; 7],
           stack_pointers: [Option<Stack>; 3], privilege_level: u8) -> TaskStateSegment {
        TaskStateSegment {
            buffer: RawVec::with_capacity(0x68),
            privilege_level: privilege_level,
            io_offset: 0, // don't handle this right now
            interrupt_stack_table: interrupt_stack_table,
            stack_pointers: stack_pointers
        }
    }

    fn get_register(&self) -> TSSDescriptor {
        TSSDescriptor {
            base: self.buffer.ptr() as u64,
            size: 0x68,
            privilege_level: self.privilege_level,
            busy: false
        }
    }

    unsafe fn save(&mut self) -> *mut u8 {
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
                stack.get() as u64
            } else {
                0
            }
        }).collect();
        ptr::copy(ptrs.as_ptr(), tss as *mut u64, 3);
        tss = tss.offset(core::u64::BYTES as isize * 3);

        // reserved
        ptr::copy([0u8; 8].as_ptr(), tss, 8);
        tss = tss.offset(8);

        // interrupt stack table
        let ptrs: Vec<u64> = self.interrupt_stack_table.iter().map(|stack| {
            if let &Some(ref stack) = stack {
                stack.get() as u64
            } else {
                0
            }
        }).collect();
        ptr::copy(ptrs.as_ptr(), tss as *mut u64, 7);
        tss = tss.offset(core::u64::BYTES as isize * 7);

        // reserved
        ptr::copy([0u8; 10].as_ptr(), tss, 10);
        tss = tss.offset(10);

        // i/o map offset
        *(tss as *mut u16).as_mut().unwrap() = self.io_offset;
        // done
    }
}

impl InterruptDescriptorTable {
    fn new(descriptors: Vec<IDTDescriptor>) -> InterruptDescriptorTable {
        InterruptDescriptorTable {
            buffer: RawVec::new(),
            descriptors: descriptors
        }
    }

    unsafe fn install(&mut self) {
        static mut REGISTER: IDTRegister = IDTRegister {
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

    unsafe fn save(&mut self) -> IDTRegister {
        let len = self.descriptors.len();

        if len == 0 {
            // do nothing if we have no descriptors
            return IDTRegister {
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

    unsafe fn copy_to(&self, idt: *mut u8) -> IDTRegister {
        let top = idt as u64;
        let mut idt = idt as *mut u64;

        for desc in self.descriptors.iter() {
            trace!("{:?}", desc);
            ptr::copy(desc.as_entry().as_ptr(), idt, 2);
            idt = idt.offset(2);
        }

        IDTRegister {
            size: (idt as u64 - top - 1) as u16,
            base: top
        }
    }
}

impl GlobalDescriptorTable {
    fn new(tss: Vec<TaskStateSegment>) -> GlobalDescriptorTable {
        GlobalDescriptorTable {
            buffer: RawVec::new(),
            tss: tss
        }
    }

    unsafe fn set_task(&mut self, task_index: u16) {
        assert!((task_index as usize) < self.tss.len());

        // task selector has to be indirected through a memory location
        asm!("ltr $0"
             :: "r"((task_index + 3) << 3)
             :: "volatile", "intel"); // could modify self
    }

    unsafe fn install(&mut self) {
        // lgdt needs a compile-time constant location
        // we basically have to use a mutable static for this
        static mut REGISTER: GDTRegister = GDTRegister {
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

    unsafe fn save(&mut self) -> GDTRegister {
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

    unsafe fn copy_to(&self, gdt: *mut u8) -> GDTRegister {
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

        GDTRegister {
            size: (gdt as u64 - top - 1) as u16,
            base: top
        }
    }
}

unsafe fn parse_elf(ptr: *const u32) {
    let info = (ptr as *const MBElfSymTag).as_ref().unwrap();

    let sections = slice::from_raw_parts((ptr as *const MBElfSymTag).offset(1) as *const elf::SectionHeader, info.num as usize);

    let mut sum: u64 = 0;

    for section in sections {
        if section.flags.0 & elf::SHF_ALLOC.0 == elf::SHF_ALLOC.0 {
            sum += section.size;
        }
    }

    info!("{:#x} bytes used by kernel", sum);
}

unsafe fn parse_cmdline(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    let cmdline = match str::from_utf8(str_slice) {
        Ok(s) => s,
        Err(e) => {
            warn!("Unable to decode boot command line: {}", e);
            return;
        }
    };

    info!("Command line: {}", cmdline);

    let mut acc = format!("");
    let mut item: Option<String> = None;

    for ch in cmdline.chars() {
        match ch {
            '=' => {
                if acc == "log" {
                    item = Some(acc);
                }

                acc = format!("");
            },
            ' ' => {
                if let Some(ref item) = item {
                    parse_command(item, &acc);
                }

                item = None;
                acc.clear();
            },
            ch => {
                acc.push(ch);
            }
        }
    }

    if let Some(ref item) = item {
        parse_command(item, &acc);
    }
}

fn parse_command(item: &String, value: &String) {
    if item == "log" {
        if value == "any" || value == "ANY" {
            log::set_level(None);
        } else if value == "critical" || value == "CRITICAL" {
            log::set_level(Some(0));
        } else if value == "error" || value == "ERROR" {
            log::set_level(Some(1));
        } else if value == "warn" || value == "WARN" {
            log::set_level(Some(2));
        } else if value == "info" || value == "INFO" {
            log::set_level(Some(3));
        } else if value == "debug" || value == "DEBUG" {
            log::set_level(Some(4));
        } else if value == "trace" || value == "TRACE" {
            log::set_level(Some(5));
        } else {
            warn!("Unknown log level: {}", value);
        }
    }
}

unsafe fn parse_bootloader(ptr: *const u32) {
    let str_ptr = ptr.offset(2) as *const u8;
    let mut size: isize = 0;

    while *str_ptr.offset(size).as_ref().unwrap() != 0 {
        size += 1;
    }

    let str_slice = slice::from_raw_parts(str_ptr, size as usize);

    match str::from_utf8(str_slice) {
        Ok(s) => {
            info!("Booted from: {}", s);
        }
        Err(e) => {
            warn!("Unable to decode bootloader name: {}", e);
        }
    }
}

unsafe fn parse_memory(ptr: *const u32) {
    // memory map
    let entry_size = *ptr.offset(2).as_ref().unwrap();
    let mut entry_ptr = ptr.offset(4) as *const MBInfoMemTag;
    let entry_end = (entry_ptr as usize + *ptr.offset(1) as usize) as *const _;

    let image_begin = &_image_begin as *const _ as usize;
    let image_end = &_image_end as *const _ as usize;

    while entry_ptr < entry_end {
        let entry = entry_ptr.as_ref().unwrap();
        match entry.addr_type {
            1 => {
                info!("RAM: {:16x} - {:16x} available",
                      entry.base_addr,
                      entry.base_addr + entry.length);
                // register memory
                let base_addr = if entry.base_addr == 0 {
                    1
                } else {
                    entry.base_addr as usize
                };

                if image_begin <= base_addr
                    && base_addr <= image_end
                    && base_addr + entry.length as usize > image_end {
                    memory::register(image_end as *mut memory::Opaque,
                                     base_addr + entry.length as usize - image_end);
                } else if base_addr < image_begin
                    && image_end > base_addr + entry.length as usize
                    && base_addr + entry.length as usize > image_begin {
                    memory::register(image_begin as *mut memory::Opaque,
                                     base_addr - image_begin);
                } else if base_addr + (entry.length as usize) < image_begin {
                    memory::register(base_addr as *mut memory::Opaque, entry.length as usize);
                } else {
                    // do not register the section
                }
            }
            3 => {
                info!("RAM: {:16x} - {:16x} ACPI",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            4 => {
                info!("RAM: {:16x} - {:16x} reserved, preserve",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
            _ => {
                info!("RAM: {:16x} - {:16x} reserved",
                      entry.base_addr,
                      entry.base_addr + entry.length);
            }
        }

        entry_ptr = align(entry_ptr as usize + entry_size as usize, 8) as *const _;
    }
}

unsafe fn parse_multiboot_tags(boot_info: *const u32) {
    // read multiboot info
    let mut ptr: *const u32 = boot_info;

    let total_size: u32 = *ptr.as_ref().unwrap();

    let end: *const u32 = (ptr as usize + total_size as usize) as *const _;

    ptr = align(ptr.offset(2) as usize, 8) as *const _;

    while ptr < end {
        match *ptr.as_ref().unwrap() {
            0 => {
                trace!("End of tags");
                break;
            }
            1 => {
                parse_cmdline(ptr);
            }
            2 => {
                parse_bootloader(ptr);
            }
            6 => {
                parse_memory(ptr);
            }
            9 => {
                parse_elf(ptr);
            }
            _ => {
                // unknown tags aren't a huge issue
                trace!("Found multiboot info tag {}", *ptr.as_ref().unwrap());
            }
        }

        // advance to the next tag
        ptr = align(ptr as usize + *ptr.offset(1).as_ref().unwrap() as usize, 8) as *const _;
    }

}

extern "C" {
    fn _interrupt() -> !;
}

#[no_mangle]
pub extern "C" fn interrupt(error_code: u64, rip: u64, cs: u64, rflags: u64, rsp: u64, ss: u64) {
    debug!("error_code: {}", error_code);
    debug!("rip: {:x}", rip);
    debug!("cs: {:x}", cs);
    debug!("rflags: {:x}", rflags);
    debug!("rsp: {:x}", rsp);
    debug!("ss: {:x}", ss);
    panic!("Interrupt");
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_info: *const u32) -> ! {
    // kernel main
    info!("Hello!");

    debug!("Multiboot info at: {:#x}", boot_info as usize);

    unsafe {
        parse_multiboot_tags(boot_info);
    }

    debug!("Done parsing tags");

    // once we're done with multiboot info, we can safely exit reserve memory
    memory::exit_reserved();

    debug!("Out of reserve memory");

    // create a new GDT with a TSS
    let tss = TaskStateSegment::new([None, None, None, None, None, None, None],
                                    [Some(Stack::create(0x10000)), None, None], 0);

    let mut gdt = GlobalDescriptorTable::new(vec![tss]);

    debug!("Created new GDT");

    unsafe {
        // install the gdt
        gdt.install();

        debug!("Installed GDT");

        // set the task
        gdt.set_task(0);

        debug!("Set new task");
    }

    let mut descriptors = vec![];

    for _ in 0..3 {
        descriptors.push(IDTDescriptor::placeholder());
    }

    descriptors.push(IDTDescriptor::new(_interrupt as u64, true, 0));

    let mut idt = InterruptDescriptorTable::new(descriptors);

    debug!("Created IDT");

    unsafe {
        idt.install();

        debug!("Installed IDT");

        asm!("sti; int 3" :::: "intel");
    }

    unreachable!("kernel_main tried to return");
}

#[cold]
#[inline(never)]
#[lang = "eh_personality"]
extern "C" fn eh_personality() {
    unreachable!("C++ exception code called")
}

#[cold]
#[inline(never)]
#[no_mangle]
#[lang = "panic_fmt"]
pub extern "C" fn rust_begin_unwind(msg: fmt::Arguments, file: &'static str, line: u32) -> ! {
    // enter reserve memory
    memory::enter_reserved();

    let loc = log::Location {
        module_path: module_path!(),
        file: file,
        line: line
    };

    log::log(0, &loc, module_path!(), msg);

    // clear interrupts and halt
    // processory must be reset to continue
    loop {
        unsafe {
            asm!("cli; hlt" ::::);
        }
    }
}

#[cold]
#[inline(never)]
#[no_mangle]
#[allow(non_snake_case)]
#[unwind]
#[lang = "eh_unwind_resume"]
pub fn _Unwind_Resume() {
    unreachable!("C++ exception code called");
}

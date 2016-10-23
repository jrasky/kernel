#![feature(asm)]
#![feature(const_fn)]
#![no_std]
extern crate core as std;

use std::ops::*;
use std::cmp::{Eq, Ord};

use std::fmt;
use std::mem;
use std::slice;

pub mod util;
pub mod error;

pub const RESERVE_SLAB_SIZE: usize = 0x1000; // four pages
pub const RESERVE_MAGIC: u64 = 15297541685404970074;
pub const VGA_BUFFER_WIDTH: usize = 80;
pub const VGA_BUFFER_HEIGHT: usize = 25;
pub const VGA_BUFFER_ADDR: usize = 0xb8000;
pub const SIMPLE_MAGIC: u64 = 4128539181889869321;
pub const STACK_SIZE: usize = 0xf000;
pub const SYSENTER_CS_MSR: u32 = 0x174;
pub const SYSENTER_EIP_MSR: u32 = 0x176;
pub const SYSENTER_ESP_MSR: u32 = 0x175;
pub const STAR_MSR: u32 = 0xC0000081;
pub const LSTAR_MSR: u32 = 0xC0000082;
pub const FMASK_MSR: u32 = 0xC0000084;
pub const EFER_MSR: u32 = 0xC0000080;
pub const CORE_CS: u16 = 0x08;
pub const CORE_DS: u16 = 0x10;
pub const CORE_SS: u16 = 0x10;

pub const COM1: u16 = 0x3f8;
pub const BOOT_INFO_MAGIC: u64 = 9390519679394335664;

pub const MULTIBOOT2_MAGIC: u32 = 0x36D76289;
pub const MULTIBOOT_MEMORY_AVAILABLE: u32 = 1;
pub const MULTIBOOT_MEMORY_RESERVED: u32 = 2;
pub const MULTIBOOT_MEMORY_ACPI_RECLAIMABLE: u32 = 3;
pub const MULTIBOOT_MEMORY_NVS: u32 = 4;
pub const MULTIBOOT_MEMORY_BADRAM: u32 = 5;
pub const MULTIBOOT_MEMORY_PERSISTENT: u32 = 7;
pub const MULTIBOOT_MEMORY_PERSISTENT_LEGACY: u32 = 12;
pub const MULTIBOOT_MEMORY_COREBOOT_TABLES: u32 = 16;
pub const MULTIBOOT_MEMORY_CODE: u32 = 20;

pub const U64_BYTES: usize = 0x8;
pub const FXSAVE_SIZE: usize = 0x200;

pub const CORE_BEGIN: u64 = 0xffffffff80000000;
pub const CORE_SIZE: usize = 0x80000000;
pub const HEAP_BEGIN: u64 = 0xffffffff81000000;
pub const IDENTITY_END: usize = 0x200000;
pub const OPTIMISTIC_HEAP: usize = 0x200000;
pub const OPTIMISTIC_HEAP_SIZE: usize = 0x200000;

pub const TASK_BEGIN: usize = 0x400000;
pub const TASK_SIZE: usize = 0x7fc00000;

pub const PAGE_TABLES_OFFSET: usize = 0x180000;

pub const STAGE1_ELF: &'static str = "target/stage1.elf";

pub const RAW_OUTPUT: &'static str = "target/gen/page_tables.bin";
pub const SEG_OUTPUT: &'static str = "target/gen/segments.bin";
pub const ASM_OUTPUT: &'static str = "target/gen/page_tables.asm";
pub const MODULE_PREFIX: &'static str = "target/modules";

pub const CANONICAL_BITS: usize = 48;
pub const PAGE_ADDR_MASK: u64 = ((1 << CANONICAL_BITS) - 1) & !((1 << 12) - 1);

#[allow(non_camel_case_types)]
pub enum c_void {}

pub trait AsBytes: Copy {
    fn as_raw_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>())
        }
    }
}

impl<T: Copy> AsBytes for T {}

#[derive(Debug)]
pub struct ByteHex<'a> {
    slice: &'a [u8]
}

impl<'a> ByteHex<'a> {
    pub const fn new(slice: &'a [u8]) -> ByteHex<'a> {
        ByteHex {
            slice: slice
        }
    }
}

impl<'a> From<&'a [u8]> for ByteHex<'a> {
    fn from(slice: &'a [u8]) -> ByteHex<'a> {
        ByteHex::new(slice)
    }
}

impl<'a> fmt::Display for ByteHex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self, f)
    }
}

impl<'a> fmt::LowerHex for ByteHex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.slice.iter() {
            try!(write!(f, "{:x}", byte));
        }

        Ok(())
    }
}

impl<'a> fmt::UpperHex for ByteHex<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.slice.iter() {
            try!(write!(f, "{:X}", byte));
        }

        Ok(())
    }
}

#[inline]
pub fn align<T>(n: T, to: T) -> T
    where T: Add<Output=T> + Sub<Output=T> + BitAnd<Output=T> + Not<Output=T> + Copy + From<u8> {
    (n + to - 1.into()) & !(to - 1.into())
}

#[inline]
pub fn align_back<T>(n: T, to: T) -> T
    where T: Sub<Output=T> + BitAnd<Output=T> + Not<Output=T> + From<u8> {
    n & !(to - 1.into())
}

#[inline]
pub fn is_aligned<T>(n: T, to: T) -> bool
    where T: Sub<Output=T> + BitAnd<Output=T> + Eq + From<u8> {
    n & (to - 1.into()) == 0.into()
}

#[inline]
pub fn on_boundary<T>(base: T, end: T, align_to: T) -> bool
    where T: Add<Output=T> + Sub<Output=T> + BitAnd<Output=T> + Not<Output=T> + Ord + Copy + From<u8> {
    align(base, align_to) <= align_back(end, align_to)
}

#[inline]
pub fn canonicalize(addr: u64) -> u64 {
    addr | (0u64.wrapping_sub((addr >> (CANONICAL_BITS - 1)) & 1) << CANONICAL_BITS)
}

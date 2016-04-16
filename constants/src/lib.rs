#![feature(reflect_marker)]
#![feature(asm)]
#![feature(const_fn)]
#![no_std]
extern crate core as std;

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
pub const MULTIBOOT2_MAGIC: u32 = 0x36D76289;

pub const U64_BYTES: usize = 0x8;
pub const FXSAVE_SIZE: usize = 0x200;

#[cfg(target_pointer_width = "64")]
pub const CORE_BEGIN: usize = 0xffffffff80000000;
pub const CORE_SIZE: usize = 0x80000000;
#[cfg(target_pointer_width = "64")]
pub const HEAP_BEGIN: usize = 0xffffffff81000000;
pub const IDENTITY_END: usize = 0x200000;
pub const OPTIMISTIC_HEAP: usize = 0x200000;

pub const TASK_BEGIN: usize = 0x400000;
pub const TASK_SIZE: usize = 0x7fc00000;

pub const PAGE_TABLES_OFFSET: usize = 0x180000;

pub const STAGE1_ELF: &'static str = "target/stage1.elf";

pub const RAW_OUTPUT: &'static str = "target/gen/page_tables.bin";
pub const SEG_OUTPUT: &'static str = "target/gen/segments.bin";
pub const ASM_OUTPUT: &'static str = "target/gen/page_tables.asm";

pub const CANONICAL_BITS: usize = 48;
pub const PAGE_ADDR_MASK: u64 = ((1 << CANONICAL_BITS) - 1) & !((1 << 12) - 1);

#[inline]
pub const fn align(n: usize, to: usize) -> usize {
    (n + to - 1) & !(to - 1)
}

#[inline]
pub const fn align_back(n: usize, to: usize) -> usize {
    n & !(to - 1)
}

#[inline]
pub const fn is_aligned(n: usize, to: usize) -> bool {
    n & (to - 1) == 0
}

#[inline]
pub fn on_boundary(base: usize, end: usize, align_to: usize) -> bool {
    align(base, align_to) <= align_back(end, align_to)
}

#[cfg(target_pointer_width = "64")]
#[inline]
pub fn canonicalize(addr: usize) -> usize {
    addr | (0usize.wrapping_sub((addr >> (CANONICAL_BITS - 1)) & 1) << CANONICAL_BITS)
}

#[cfg(target_pointer_width = "32")]
#[inline]
pub const fn canonicalize(addr: usize) -> usize {
    addr
}

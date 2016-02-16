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
pub const EFER_MSR: u32 = 0xC0000080;
pub const CORE_CS: u16 = 0x08;
#[allow(dead_code)] // will use eventually
pub const CORE_DS: u16 = 0x10;
#[allow(dead_code)] // will use eventually
pub const CORE_SS: u16 = 0x10;

pub const U64_BYTES: usize = 0x8;
pub const FXSAVE_SIZE: usize = 0x200;

pub const HEAP_BEGIN: usize = 0xffffffff81000000;

#[inline]
pub const fn align(n: usize, to: usize) -> usize {
    (n + to - 1) & !(to - 1)
}

#[inline]
pub const fn align_back(n: usize, to: usize) -> usize {
    n & !(to - 1)
}

#[inline]
#[allow(dead_code)] // might use eventually
pub const fn is_aligned(n: usize, to: usize) -> bool {
    n & (to - 1) == 0
}

#[inline]
#[allow(dead_code)] // will use eventually
pub fn on_boundary(base: usize, end: usize, align_to: usize) -> bool {
    align(base, align_to) <= align_back(end, align_to)
}

pub const RESERVE_SLAB_SIZE: usize = 0x1000; // four pages
pub const RESERVE_MAGIC: u64 = 15297541685404970074;
pub const VGA_BUFFER_WIDTH: usize = 80;
pub const VGA_BUFFER_HEIGHT: usize = 25;
pub const VGA_BUFFER_ADDR: usize = 0xb8000;
pub const SIMPLE_MAGIC: u64 = 4128539181889869321;
pub const STACK_SIZE: usize = 0xf000;

#[inline]
pub const fn align(n: usize, to: usize) -> usize {
    (n + to - 1) & !(to - 1)
}


pub const CANONICAL_BITS: usize = 48;
pub const U64_BYTES: usize = 8;
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
#[allow(dead_code)] // will use eventually
pub fn on_boundary(base: usize, end: usize, align_to: usize) -> bool {
    align(base, align_to) <= align_back(end, align_to)
}

#[inline]
pub fn canonicalize(addr: usize) -> usize {
    addr | (0usize.wrapping_sub((addr >> (CANONICAL_BITS - 1)) & 1) << CANONICAL_BITS)
}

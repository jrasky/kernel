#[cfg(not(test))]
pub use core::fmt::Debug;
#[cfg(test)]
pub use std::fmt::Debug;

#[cfg(test)]
pub use std::fmt;
#[cfg(not(test))]
pub use core::fmt;

#[cfg(not(test))]
use core::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};

#[cfg(test)]
use std::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};

use collections::BTreeSet;
use collections::Bound::{Excluded, Unbounded};

use constants::*;
use constants;

#[derive(Clone, Copy)]
pub struct Region {
    base: usize,
    size: usize
}

#[derive(Debug, Clone)]
pub struct Allocator {
    free: BTreeSet<Region>,
    used: BTreeSet<Region>
}

impl Debug for Region {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Region {{ base: 0x{:x}, size: 0x{:x} }}", self.base, self.size)
    }
}

impl Region {
    pub const fn new(base: usize, size: usize) -> Region {
        Region {
            base: base,
            size: size
        }
    }

    #[inline]
    pub fn base(&self) -> usize {
        self.base
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    #[inline]
    pub fn before(&self, other: &Region) -> bool {
        self.base + self.size == other.base
    }

    #[inline]
    pub fn after(&self, other: &Region) -> bool {
        other.base + other.size == self.base
    }
}

impl Ord for Region {
    fn cmp(&self, other: &Region) -> Ordering {
        if self.base + self.size <= other.base || self.base >= other.base + other.size {
            self.base.cmp(&other.base)
        } else {
            Ordering::Equal
        }
    }
}

impl PartialOrd for Region {
    #[inline]
    fn partial_cmp(&self, other: &Region) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Region {
    #[inline]
    fn eq(&self, other: &Region) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Region {}

impl Allocator {
    pub fn new() -> Allocator {
        Allocator {
            free: BTreeSet::new(),
            used: BTreeSet::new()
        }
    }

    pub fn set_used(&mut self, region: Region) -> bool {
        if self.free.contains(&region) {
            return false;
        }

        let mut last_region = None;
        let mut next_region = None;

        if let Some(last) = self.used.range(Unbounded, Excluded(&region)).next_back() {
            if region.after(&last) {
                last_region = Some(last.clone());
            }
        }
        
        if let Some(next) = self.used.range(Excluded(&region), Unbounded).next() {
            if region.before(&next) {
                next_region = Some(next.clone());
            }
        }

        let base = if let Some(ref last) = last_region {
            assert!(self.used.remove(last));
            last.base()
        } else {
            region.base()
        };

        let size = if let Some(ref next) = next_region {
            assert!(self.used.remove(next));
            next.size() + (next.base() - base)
        } else {
            region.size() + (region.base() - base)
        };

        assert!(self.used.insert(Region::new(base, size)));

        true
    }

    pub fn register(&mut self, region: Region) -> bool {
        trace!("{:?}", region);

        if self.used.contains(&region) {
            return false;
        }

        let mut last_region = None;
        let mut next_region = None;

        // try to connect segments together
        if let Some(last) = self.free.range(Unbounded, Excluded(&region)).next_back() {
            trace!("{:?}", last);
            if region.after(&last) {
                // combine with last segment
                last_region = Some(last.clone());
            }
        }

        if let Some(next) = self.free.range(Excluded(&region), Unbounded).next() {
            trace!("{:?}", next);
            if region.before(&next) {
                // combine with next segment
                next_region = Some(next.clone());
            }
        }

        let base = if let Some(ref last) = last_region {
            assert!(self.free.remove(last));
            last.base()
        } else {
            region.base()
        };

        let size = if let Some(ref next) = next_region {
            assert!(self.used.remove(next));
            next.size() + (next.base() - base)
        } else {
            region.size() + (region.base() - base)
        };

        assert!(self.free.insert(Region::new(base, size)));

        true
    }

    pub fn forget(&mut self, region: Region) -> bool {
        let mut pre_region = None;
        let mut post_region = None;

        if let Some(old) = self.free.get(&region) {
            if old.base() < region.base() {
                pre_region = Some(Region::new(old.base(), region.base() - old.base()));
            }

            if old.base() + old.size() > region.base() + region.size() {
                post_region =
                    Some(Region::new(old.base() + old.size(),
                                     (old.base() + old.size()) - (region.base() + region.size())));
            }
        }

        if !self.free.remove(&region) {
            false
        } else {
            if let Some(pre) = pre_region {
                self.free.insert(pre);
            }

            if let Some(post) = post_region {
                self.free.insert(post);
            }

            true
        }
    }

    pub fn allocate(&mut self, size: usize, align: usize) -> Option<Region> {
        let mut selected_region = None;

        for region in self.free.iter() {
            if region.size() >= size {
                selected_region = Some(region.clone());
                break;
            }
        }

        if let Some(region) = selected_region {
            assert!(self.free.remove(&region));
            let aligned_base = constants::align(region.base(), align);
            if aligned_base - region.base() > 0 {
                assert!(self.free.insert(Region::new(region.base(), aligned_base - region.base())));
            }

            let new_region = Region::new(aligned_base, size);

            assert!(self.used.insert(new_region));
            assert!(self.free.insert(Region::new(region.base() + size, region.size() - size)));
            Some(new_region)
        } else {
            None
        }
    }

    pub fn release(&mut self, region: Region) -> bool {
        if !self.used.remove(&region) {
            return false;
        } else {
            self.register(region)
        }
    }

    pub fn grow(&mut self, region: Region, size: usize) -> bool {
        debug_assert!(size > region.size(), "Size was not greater than on grow");

        let mut next_region = None;

        if let Some(next) = self.free.range(Excluded(&region), Unbounded).next() {
            if next.size() >= size - region.size() {
                next_region = Some(next.clone());
            }
        } else {
            return false;
        }

        if let Some(next) = next_region {
            assert!(self.free.remove(&next));

            let new_region = Region::new(region.base() + region.size(),
                                         region.size() + next.size() - size);

            assert!(self.free.insert(new_region));

            true
        } else {
            false
        }
    }

    pub fn shrink(&mut self, region: Region, size: usize) -> bool {
        debug_assert!(size < region.size(), "Size was not smaller than on shrink");

        self.register(Region::new(region.base() + size, region.size() - size));

        true
    }

    pub fn resize(&mut self, region: Region, size: usize, align: usize) -> Option<Region> {
        if region.size() == size {
            return Some(region);
        }

        if size < region.size() && self.shrink(region, size) {
            return Some(Region::new(region.base(), size));
        } else if self.grow(region, size) {
            return Some(Region::new(region.base(), size));
        }

        if !self.release(region) {
            return None;
        }

        if let Some(new_region) = self.allocate(size, align) {
            Some(new_region)
        } else {
            assert!(self.forget(region));
            assert!(self.used.insert(region));
            None
        }
    }
}

#[cfg(test)]
#[test]
fn test_allocator() {
    let mut allocator = Allocator::new();

    assert!(allocator.register(Region::new(0x200000, 0x200000)));
    assert!(allocator.allocate(0x1000, 0x1000).is_some());
}

#[cfg(test)]
#[test]
fn test_allocator_limit() {
    let mut allocator = Allocator::new();

    assert!(allocator.register(Region::new(0xfffffff810000000, 0x7f0000000)));
}

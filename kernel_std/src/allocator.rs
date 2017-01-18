use std::fmt::Debug;
use std::iter::Iterator;

use std::fmt;
use std::str;

use collections::BTreeMap;

use constants;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Region {
    base: u64,
    size: u64
}

#[derive(Debug, Clone)]
struct AddressSpace {
    by_base: BTreeMap<u64, Region>,
    by_end: BTreeMap<u64, Region>
}

#[derive(Debug, Clone)]
pub struct Allocator {
    free: AddressSpace,
    used: AddressSpace
}

impl AddressSpace {
    fn new() -> AddressSpace {
        AddressSpace {
            by_base: BTreeMap::new(),
            by_end: BTreeMap::new()
        }
    }

    fn iter(&self) -> ::collections::btree_map::Values<u64, Region> {
        self.by_base.values()
    }

    fn contains(&self, region: Region) -> bool {
        if self.by_base.range(region.base()..region.end()).next().is_some()
            || self.by_end.range(region.base() + 1..region.end() + 1).next().is_some() {
                // return early if we find any region that begins or ends in our range
                return true;
            }

        // one case left to consider: a region that entirely contains our region
        if let Some((_, containing_region)) = self.by_base.range(..region.base()).last() {
            if containing_region.end() > region.base() {
                // this region overlaps
                return true;
            }
        }

        // otherwise no overlaps
        false
    }

    fn remove(&mut self, region: Region) -> bool {
        // exact remove region
        if let Some(base_region) = self.by_base.get(&region.base()) {
            if let Some(end_region) = self.by_end.get(&region.end()) {
                if base_region != end_region {
                    return false;
                }
            } else {
                return false;
            }
        } else {
            return false;
        }

        // getting to this point means we can remove the region
        assert!(self.by_base.remove(&region.base()).is_some());
        assert!(self.by_end.remove(&region.end()).is_some());

        true
    }

    fn insert(&mut self, region: Region) -> bool {
        if self.contains(region) {
            // can't insert, this region overlaps
            return false;
        }

        // add an entry in both the base and end list
        assert!(self.by_base.insert(region.base(), region).is_none());
        // - 1 for the last addressable byte in this region
        assert!(self.by_end.insert(region.end(), region).is_none());

        true
    }

    fn remove_all(&mut self, region: Region) {
        // remove/truncate all overlapping regions
        self.remove_contained(region);

        let last_before = self.by_base.range(..region.base()).last().map(|(_, v)| *v);
        let first_after = self.by_end.range(region.end()..).nth(0).map(|(_, v)| *v);

        if last_before.is_some() && last_before == first_after {
            self.split_region(last_before.unwrap(), region);
        } else {
            if let Some(last_before) = last_before {
                self.shrink_before_region(last_before, region);
            }

            if let Some(first_after) = first_after {
                self.shrink_after_region(first_after, region);
            }
        }
    }

    fn shrink_before_region(&mut self, container: Region, region: Region) {
        // shrink a region to before the given region
        let piece = Region::new(container.base(), region.base() - container.base());

        assert!(self.remove(container));
        assert!(self.insert(piece));
    }

    fn shrink_after_region(&mut self, container: Region, region: Region) {
        // truncate a region to being after the given region
        let piece = Region::new(region.end(), container.end() - region.end());

        assert!(self.remove(container));
        assert!(self.insert(piece));
    }

    fn split_region(&mut self, container: Region, region: Region) {
        // split container into two peices around region
        let piece_before = Region::new(container.base(), region.base() - container.base());
        let piece_after = Region::new(region.end(), container.end() - region.end());

        assert!(self.remove(container));
        assert!(self.insert(piece_before));
        assert!(self.insert(piece_after));
    }

    fn remove_contained(&mut self, region: Region) {
        // remove all regions that are completely contained by the given region

        let mut to_remove = vec![];

        for (_, other_region) in self.by_base.range(region.base()..region.end()) {
            if region.contains(other_region) {
                to_remove.push(*other_region);
            }
        }

        for region in to_remove {
            assert!(self.remove(region));
        }
    }
}

impl Debug for Region {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Region {{ base: 0x{:x}, size: 0x{:x} }}", self.base, self.size)
    }
}

impl Region {
    pub const fn new(base: u64, size: u64) -> Region {
        Region {
            base: base,
            size: size
        }
    }

    #[inline]
    pub fn aligned_size(&self, align: u64) -> u64 {
        let aligned_base = constants::align(self.base, align);

        if aligned_base > self.end() {
            0
        } else {
            self.end() - aligned_base
        }
    }

    #[inline]
    pub fn contains(&self, other: &Region) -> bool {
        other.base() >= self.base() && other.end() <= self.end()
    }

    #[inline]
    pub fn base(&self) -> u64 {
        self.base
    }

    #[inline]
    pub fn end(&self) -> u64 {
        self.base + self.size
    }

    #[inline]
    pub fn size(&self) -> u64 {
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

impl Allocator {
    pub fn new() -> Allocator {
        Allocator {
            free: AddressSpace::new(),
            used: AddressSpace::new()
        }
    }

    pub fn set_used(&mut self, region: Region) -> bool {
        trace!("{:?}", region);

        if self.free.contains(region) {
            return false;
        }

        assert!(self.used.insert(region));

        true
    }

    pub fn register(&mut self, region: Region) -> bool {
        trace!("{:?}", region);

        if self.used.contains(region) {
            return false;
        }

        assert!(self.free.insert(region));

        true
    }

    pub fn forget(&mut self, region: Region) -> bool {
        trace!("{:?}", region);

        if !self.free.contains(region) {
            return false;
        }

        self.free.remove_all(region);

        true
    }

    pub fn allocate(&mut self, size: u64, align: u64) -> Option<Region> {
        trace!("size 0x{:x}, align 0x{:x}", size, align);
        let mut selected_region = None;

        for region in self.free.iter() {
            if region.aligned_size(align) >= size {
                selected_region = Some(region.clone());
                break;
            }
        }

        if let Some(region) = selected_region {
            let aligned_base = constants::align(region.base(), align);
            let new_region = Region::new(aligned_base, size);

            assert!(self.used.insert(new_region));
            self.free.remove_all(new_region);

            Some(new_region)
        } else {
            None
        }
    }

    pub fn release(&mut self, region: Region) -> bool {
        if !self.used.contains(region) {
            return false;
        }

        assert!(self.used.remove(region));

        self.free.insert(region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0x200000, 0x200000)));
        assert!(allocator.allocate(0x1000, 0x1000).is_some());
        assert!(allocator.allocate(0x1000, 0x1000).is_some());
    }

    #[test]
    fn test_allocator_limit() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0xfffffff810000000, 0x7e0000000)));
    }

    #[test]
    fn test_multiple_overlap() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0x0, 0x1000)));
        assert!(allocator.register(Region::new(0x2000, 0x1000)));
        assert!(allocator.register(Region::new(0x4000, 0x1000)));
        assert!(allocator.register(Region::new(0x6000, 0x1000)));
        assert!(allocator.register(Region::new(0x8000, 0x1000)));

        assert!(allocator.forget(Region::new(0x0, 0x10000)));

        assert!(allocator.allocate(0x1, 0x1).is_none());
    }
}

use std::fmt::Debug;
use std::iter::Iterator;

use std::fmt;
use std::cmp;
use std::str;

use collections::BTreeSet;
use collections::Bound::{Unbounded, Excluded, Included};

use constants;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Region {
    base: u64,
    size: u64
}

struct AddressSpace {
    by_base: BTreeMap<u64, Region>,
    by_end: BTreeMap<u64, Region>
}

#[derive(Debug, Clone)]
pub struct Allocator {
    free: BTreeSet<Region>,
    used: BTreeSet<Region>
}

impl AddressSpace {
    fn new() -> AddressSpace {
        AddressSpace {
            by_base: BTreeMap::new(),
            by_end: BTreeMap::new()
        }
    }

    fn check_overlap(&self, region: Region) -> bool {
        if self.by_base.range(Included(region.base()), Excluded(region.end())).next().is_some()
            || self.by_end.range(Included(region.base()), Excluded(region.end())).next().is_some() {
                // return early if we find any region that begins or ends in our range
                return true;
            }

        // one case left to consider: a region that entirely contains our region
        if let Some((_, containing_region)) = self.by_base.range(Unbounded, Included(region.base())).last() {
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
            if let Some(end_region) = self.by_end.get(&(region.end() - 1)) {
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
        assert!(self.by_base.remove(&region.base()));
        assert!(self.by_end.remove(&(region.end() - 1)));

        true
    }

    fn insert(&mut self, region: Region) -> bool {
        if self.check_overlap(region) {
            // can't insert, this region overlaps
            return false;
        }

        // add an entry in both the base and end list
        assert!(self.by_base.insert(region.base(), region));
        // - 1 for the last addressable byte in this region
        assert!(self.by_end.insert(region.base() + region.end() - 1, region));

        true
    }

    fn remove_all(&mut self, region: Region) {
        // remove all regions that overlap with the given region
        let mut base_insert = None;
        let mut end_insert = None;

        let mut to_remove = vec![];

        {
            let mut base_range = self.by_base.range(Unbounded, Excluded(region.end()));

            if let Some((key, base_region)) = base_range.next_back() {
                base_insert = Some(base_region);

                to_remove.push(key);
            }

            for (key, _) in base_range {
                to_remove.push(key);
            }
        }

        for key in to_remove {
            assert!(self.by_base.remove(&key));
        }

        let mut to_remove = vec![];

        {
            let mut end_range = self.by_end.range(Included(region.base()), Unbounded);

            if let Some((key, end_region)) = end_range.next() {
                end_insert = Some(end_region);

                to_remove.push(key);
            }

            for (key, _) in end_range {
                to_remove.push(key);
            }
        }

        for key in to_remove {
            assert!(self.by_base.remove(&key));
        }

        if !base_insert.is_none() && base_insert == end_insert {
            // TODO keep working on this
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
            free: BTreeSet::new(),
            used: BTreeSet::new()
        }
    }

    pub fn set_used(&mut self, region: Region) -> bool {
        trace!("{:?}", region);

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
        trace!("{:?}", region);
        let mut pre_region = None;
        let mut post_region = None;

        if let Some(old) = self.free.get(&region) {
            if old.base() < region.base() {
                pre_region = Some(Region::new(old.base(), region.base() - old.base()));
            }

            if old.base() + old.size() > region.base() + region.size() {
                post_region =
                    Some(Region::new(region.base() + region.size(),
                                     (old.base() + old.size()) - (region.base() + region.size())));
            }
        }

        if !self.free.remove(&region) {
            false
        } else {
            trace!("{:?}, {:?}", pre_region, post_region);
            if let Some(pre) = pre_region {
                self.free.insert(pre);
            }

            if let Some(post) = post_region {
                self.free.insert(post);
            }

            true
        }
    }

    pub fn allocate(&mut self, size: u64, align: u64) -> Option<Region> {
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

    pub fn grow(&mut self, region: Region, size: u64) -> bool {
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

    pub fn shrink(&mut self, region: Region, size: u64) -> bool {
        debug_assert!(size < region.size(), "Size was not smaller than on shrink");

        self.register(Region::new(region.base() + size, region.size() - size));

        true
    }

    pub fn resize(&mut self, region: Region, size: u64, align: u64) -> Option<Region> {
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
mod tests {
    use super::*;

    #[test]
    fn test_allocator() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0x200000, 0x200000)));
        assert!(allocator.allocate(0x1000, 0x1000).is_some());
    }

    #[test]
    fn test_allocator_limit() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0xfffffff810000000, 0x7f0000000)));
    }

    #[test]
    fn test_multiple_overlap() {
        let mut allocator = Allocator::new();

        assert!(allocator.register(Region::new(0x0, 0x1000)));
        assert!(allocator.register(Region::new(0x2000, 0x3000)));
        assert!(allocator.register(Region::new(0x4000, 0x5000)));
        assert!(allocator.register(Region::new(0x6000, 0x7000)));
        assert!(allocator.register(Region::new(0x8000, 0x9000)));

        assert!(allocator.forget(Region::new(0x0, 0x10000)));

        assert!(allocator.allocate(0x1, 0x1).is_none());
    }
}

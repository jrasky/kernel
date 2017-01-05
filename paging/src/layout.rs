use collections::BTreeSet;

use constants::*;

use kernel_std::*;

use segment::Segment;

pub struct Layout {
    map: BTreeSet<Segment>,
    free: Allocator,
}

impl Layout {
    pub fn new() -> Layout {
        Layout {
            map: BTreeSet::new(),
            free: Allocator::new()
        }
    }

    #[inline]
    pub fn allocate(&mut self, size: u64, align: u64) -> Option<Region> {
        self.free.allocate(size, align)
    }

    #[inline]
    pub fn register(&mut self, region: Region) -> bool {
        self.free.register(region)
    }

    #[inline]
    pub fn forget(&mut self, region: Region) -> bool {
        let dummy = Segment::dummy_range(region.base(), region.size());

        // only forget a region if no segments are mapped in it
        if !self.map.contains(&dummy) {
            self.free.forget(region)
        } else {
            false
        }
    }

    #[inline]
    pub fn segments(&self) -> ::collections::btree_set::Iter<Segment> {
        self.map.iter()
    }
    
    pub fn insert(&mut self, segment: Segment) -> bool {
        trace!("Inserting segment {:?}", &segment);
        if self.map.insert(segment.clone()) {
            let region = Region::new(segment.virtual_base(), segment.size());
            // may or may not already be allocated
            self.free.set_used(region);

            true
        } else {
            false
        }
    }

    pub fn remove(&mut self, segment: Segment) -> bool {
        let region = Region::new(segment.virtual_base(), segment.size());

        if self.free.release(region) && self.map.remove(&segment) {
            true
        } else {
            false
        }
    }

    pub fn to_physical(&self, addr: u64) -> Option<u64> {
        let dummy = Segment::dummy(addr);

        if let Some(segment) = self.map.get(&dummy) {
            // address has a mapping
            Some((addr & ((1 << CANONICAL_BITS) - 1)) - segment.virtual_base() + segment.physical_base())
        } else {
            // no mapping
            None
        }
    }

    pub fn to_virtual(&self, addr: u64) -> Option<u64> {
        // naive implementation
        for segment in self.map.iter() {
            if segment.physical_base() <= addr && addr <= segment.physical_base() + segment.size() {
                return Some(addr - segment.physical_base() + segment.virtual_base());
            }
        }

        None
    }
}

use std::iter::Iterator;

use collections::BTreeMap;
use collections::Bound::Included;
use collections::btree_map;

use allocator::Region;

pub struct Map {
    entries: BTreeMap<Region, Region>
}

impl Map {
    pub fn new() -> Map {
        Map {
            entries: BTreeMap::new()
        }
    }

    pub fn map(&mut self, from: Region, to: Region) -> bool {
        match self.entries.entry(from) {
            btree_map::Entry::Vacant(entry) => {
                entry.insert(to);
                true
            },
            _ => false
        }
    }

    pub fn unmap(&mut self, from: &Region) -> bool {
        self.entries.remove(from).is_some()
    }

    pub fn translate(&self, addr: u64) -> Option<u64> {
        // create a dummy region
        let dummy = Region::new(addr, 0);

        if let Some((from, to)) = self.entries.range(Included(&dummy), Included(&dummy)).next() {
            Some((to.base() as i64 + (addr as i64 - from.base() as i64)) as u64)
        } else {
            None
        }
    }
}

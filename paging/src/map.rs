use include::*;

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

    pub fn translate(&self, addr: usize) -> Option<usize> {
        // create a dummy region
        let dummy = Region::new(addr, 0);

        if let Some((from, to)) = self.entries.range(Included(&dummy), Included(&dummy)).next() {
            Some((to.base() as isize + (addr as isize - from.base() as isize)) as usize)
        } else {
            None
        }
    }
}

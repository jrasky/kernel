use include::*;

/// Access settings for physical memory
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub write: bool,
    pub user: bool,
    pub write_through: bool,
    pub cache_disable: bool,
    pub execute_disable: bool,
    pub attribute_table: bool,
    pub protection_key: u8,
    pub global: bool,
    pub size: PageSize,
    pub base: usize
}


impl Eq for Page {}

impl PartialEq for PageSize {
    fn eq(&self, other: &PageSize) -> bool {
        *self as usize == *other as usize
    }
}

impl Eq for PageSize {}

impl Ord for PageSize {
    fn cmp(&self, other: &PageSize) -> Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl PartialOrd for PageSize {
    fn partial_cmp(&self, other: &PageSize) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Page {
    pub fn get_entry(&self) -> u64 {
        let mut entry = (self.protection_key as u64) << 59 | self.base as u64 | (1 << 0);

        if self.execute_disable {
            entry |= 1 << 63;
        }

        if self.global {
            entry |= 1 << 8;
        }

        if self.size != PageSize::Page || self.attribute_table {
            entry |= 1 << 7;
        } else if self.attribute_table {
            entry |= 1 << 12;
        }

        if self.cache_disable {
            entry |= 1 << 4;
        }

        if self.write_through {
            entry |= 1 << 3;
        }

        if self.user {
            entry |= 1 << 2;
        }

        if self.write {
            entry |= 1 << 1;
        }

        trace!("Page entry: 0x{:x}", entry);

        entry
    }
}

impl PageSize {
    #[inline]
    pub fn get_shift(self) -> usize {
        (self as usize).trailing_zeros() as usize
    }
}

use include::*;

pub enum Level {
    PML4E,
    PDPTE,
    PDE,
    PTE
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Entry {
    entry: u64
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct Table {
    entries: [Entry; 0x200]
}

pub struct Info {
    pub page: bool,
    pub write: bool,
    pub execute: bool,
    pub user: bool,
    pub global: bool,
    pub write_through: bool,
    pub cache_disable: bool,
    pub protection_key: u8,
    pub level: Level,
    pub address: usize
}

pub trait Base {
    fn to_physical(&self, address: usize) -> Option<usize>;
    fn to_virtual(&self, address: usize) -> Option<usize>;
    fn allocate_table(&mut self) -> Unique<Table>;
    fn release_table(&mut self, table: Unique<Table>);
}

impl Table {
    pub fn write(&mut self, entry: Entry, idx: usize) -> Entry {
        let old = self.entries[idx];
        self.entries[idx] = entry;
        old
    }

    pub fn read(&mut self, idx: usize) -> Entry {
        self.entries[idx]
    }
}

impl From<Info> for Entry {
    fn from(info: Info) -> Entry {
        let mut entry = (self.protection_key as u64) << 59 | self.address as u64 | (1 << 0);

        if self.execute_disable {
            entry |= 1 << 63;
        }

        if self.global {
            entry |= 1 << 8;
        }

        if (self.level == Level::PTE && self.attribute_table) || self.page {
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

        Entry {
            entry: entry
        }
    }
}

impl Entry {
    pub fn address(&self, level: Level) -> usize {
        if self.is_page(level) {
            canonicalize((self.entry as usize) & PAGE_ADDR_MASK & !(1 << 12))
        } else {
            canonicalize((self.entry as usize) & PAGE_ADDR_MASK)
        }
    }

    pub fn present(&self) -> bool {
        self.entry & 1 << 0 != 0
    }

    pub fn write(&self) -> bool {
        self.entry & 1 << 1 != 0
    }

    pub fn user(&self) -> bool {
        self.entry & 1 << 2 != 0
    }

    pub fn execute(&self) -> bool {
        self.entry & 1 << 63 == 0
    }

    pub fn write_through(&self) -> bool {
        self.entry & 1 << 3 != 0
    }

    pub fn cache_disable(&self) -> bool {
        self.entry & 1 << 4 != 0
    }

    pub fn is_page(&self, level: Level) -> bool {
        match level {
            Level::PML4E => false,
            Level::PTE => true,
            _ => {
                self.entry & 1 << 7 == 0
            }
        }
    }

    pub fn accessed(&self) -> bool {
        self.entry & 1 << 5 != 0
    }

    pub fn dirty(&self) -> bool {
        self.entry & 1 << 6 != 0
    }

    pub fn global(&self, level: Level) -> bool {
        if level == Level::PML4E {
            false
        } else {
            self.entry & 1 << 8 != 0
        }
    }

    pub fn attribute_table(&self, level: Level) -> bool {
        if level == Level::PTE {
            self.entry & 1 << 7 != 0
        } else if self.is_page(level) {
            self.entry & 1 << 12 != 0
        } else {
            false
        }
    }

    pub fn protection_key(&self, level: Level) -> u8 {
        if self.is_page(level) {
            (self.entry >> 59) as u8
        }
    }
}

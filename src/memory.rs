use core::marker::Reflect;
use core::fmt::{Write, Debug, Display, Formatter};
use core::sync::atomic::{AtomicPtr, Ordering};

use core::fmt;

use error::*;

static MEMORY: MemManager = MemManager {
    free: AtomicPtr::new(0 as *mut Allocation),
    used: AtomicPtr::new(0 as *mut Allocation),
};

struct Allocation {
    base: *mut u8,
    size: usize,
    next: AtomicPtr<Allocation>,
}

struct MemManager {
    free: AtomicPtr<Allocation>,
    used: AtomicPtr<Allocation>,
}

#[derive(Debug)]
enum MemError {
    InvalidRange,
}

impl Reflect for MemManager {}
unsafe impl Sync for MemManager {}

impl Display for MemError {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        write!(fmt, "MemError: {}", self.description())
    }
}

impl Error for MemError {
    fn description(&self) -> &str {
        match self {
            &MemError::InvalidRange => "Invalid range",
        }
    }
}

impl Allocation {
    const fn new(base: *mut u8, size: usize) -> Allocation {
        Allocation {
            base: base,
            size: size,
            next: AtomicPtr::new(0 as *mut _),
        }
    }
}

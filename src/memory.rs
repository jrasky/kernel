use core::prelude::v1::*;

use core::marker::Reflect;
use core::fmt::{Write, Debug, Display, Formatter};
use core::sync::atomic::{AtomicPtr, Ordering};

use core::fmt;
use core::cmp;
use core::ptr;
use core::mem;

use error::Error;

static MEMORY: Manager = Manager { free: AtomicPtr::new(0 as *mut Block) };

struct Header {
    base: *mut u8,
    size: usize,
}

#[derive(Debug)]
struct Block {
    base: *mut u8,
    size: usize,
    next: AtomicPtr<Block>,
}

struct Manager {
    free: AtomicPtr<Block>,
}

#[derive(Debug)]
enum MemError {
    InvalidRange,
}

impl Reflect for Manager {}
unsafe impl Sync for Manager {}

impl PartialEq<Block> for Block {
    fn eq(&self, other: &Block) -> bool {
        self.base == other.base && self.size == other.size
    }
}

impl Eq for Block {}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Block) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Block {
    fn cmp(&self, other: &Block) -> cmp::Ordering {
        match self.size.cmp(&other.size) {
            cmp::Ordering::Equal => {
                self.base.cmp(&other.base)
            }
            order => order,
        }
    }
}

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

impl Block {
    const fn new(base: *mut u8, size: usize) -> Block {
        Block {
            base: base,
            size: size,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    unsafe fn at(base: *mut Block, size: usize) -> *mut Block {
        *base.as_mut().expect("Given null base") = Block::new(base as *mut _, size);
        base
    }

    const fn dummy(size: usize) -> Block {
        Block::new(ptr::null_mut(), size)
    }

    fn reduce(&mut self, by: usize) {
        assert!(by < self.size, "Tried to reduce to zero or beyond");
        self.size -= by;
        self.base = unsafe {self.base.offset(by as isize)};
    }
}

impl Manager {
    /// Get the block parent to the next biggest block in the free list, return
    /// Err with the largest block if no bigger block exists, and None if the
    /// free list is empty
    fn find(&self, target: &Block) -> Option<Result<&mut Block, &mut Block>> {
        let mut ptr = self.free.load(Ordering::Relaxed);
        if ptr.is_null() {
            trace!("Only null entries in free list");
            return None;
        }

        loop {
            let block = unsafe { ptr.as_mut() }.unwrap();
            match unsafe { block.next.load(Ordering::Relaxed).as_mut() } {
                None => {
                    trace!("Reached end of list, returning closest: {:?}", block);
                    return Some(Err(block));
                }
                Some(next) => {
                    if next as &Block >= target {
                        trace!("Found block of interest: {:?}", block);
                        return Some(Ok(block));
                    } else {
                        // continue iterating
                        ptr = next as *mut _;
                    }
                }
            }
        }
    }

    fn add_block(&self, block: *mut Block) {
        // null blocks are a bad idea
        assert!(!block.is_null(), "Tried to add null block");

        loop {
            match self.find(unsafe { block.as_ref() }.unwrap()) {
                None => {
                    // insert the first block
                    if self.free
                           .compare_and_swap(ptr::null_mut(), block, Ordering::SeqCst)
                           .is_null() {
                        debug!("Successfully added block {:?}", block);
                        break;
                    }
                }
                Some(Err(parent)) => {
                    // insert the block at the end of the list
                    if parent.next
                             .compare_and_swap(ptr::null_mut(), block, Ordering::SeqCst)
                             .is_null() {
                        debug!("Successfully added block {:?}", block);
                        break;
                    }
                }
                Some(Ok(parent)) => {
                    // first update the block
                    let value = parent.next.load(Ordering::Relaxed);
                    unsafe { block.as_mut() }.unwrap().next.store(value, Ordering::Relaxed);
                    if parent.next.compare_and_swap(value, block, Ordering::SeqCst) == value {
                        debug!("Successfully added block {:?}", block);
                        break;
                    }
                }
            }
        }
    }

    fn remove_block(&self, block: *mut Block) {
        // null blocks are a bad idea
        assert!(!block.is_null(), "Tried to remove a null block");

        loop {
            match self.find(unsafe { block.as_mut() }.unwrap()) {
                Some(Ok(parent)) => {
                    // squish the list down
                    let current = parent.next.load(Ordering::Relaxed);
                    let new_next = unsafe { current.as_ref() }
                                       .unwrap()
                                       .next
                                       .load(Ordering::Relaxed);

                    if parent.next.compare_and_swap(current, new_next, Ordering::SeqCst) ==
                       current {
                        debug!("Successfully removed block {:?}", block);
                        break;
                    }
                }
                _ => {
                    debug!("Block not found: {:?}", block);
                    break;
                }
            }
        }
    }

    fn allocate(&self, mut size: usize) -> Option<*mut u8> {
        if size == 0 {
            return None;
        }

        // add header size
        size += mem::size_of::<Header>();

        match self.find(&Block::dummy(size)) {
            None => None,
            Some(Err(_)) => None,
            Some(Ok(block)) => {
                // at this point, assume block.next is not null
                let block = unsafe {
                    block.next
                         .load(Ordering::Relaxed)
                         .as_mut()
                         .expect("Find returned terminating element")
                };
                let addr = block.base;
                self.remove_block(block as *mut _);
                if size < block.size {
                    block.reduce(size);
                    self.add_block(block);
                }
                Some(addr)
            }
        }
    }
}

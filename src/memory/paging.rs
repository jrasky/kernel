use collections::{BTreeSet, Vec};

use core::cmp::{PartialEq, Eq, Ord, PartialOrd, Ordering};
use core::ptr::Unique;

use constants::align;

use memory::Opaque;

struct Layout {
    segments: BTreeSet<Segment>
}

struct Segment {
    base: *mut Opaque,
    size: usize,
    present: bool,
    write: bool,
    execute: bool,
    user: bool
}

#[repr(usize)]
#[derive(Clone, Copy)]
enum FrameSize {
    Huge = 0x40000000, // 1 gigabyte
    Big = 0x200000,    // 2 megabytes
    Page = 0x1000      // 4 kilobytes
}

struct Frame {
    base: *mut Opaque,
    size: FrameSize,
    present: bool,
    write: bool,
    execute: bool,
    user: bool
}

struct Mapping {
    context_id: u16,
    pml4: Unique<Opaque>,
    pdpts: Vec<Unique<Opaque>>,
    pds: Vec<Unique<Opaque>>,
    pts: Vec<Unique<Opaque>>
}

// CR3
struct Structure {
    context_id: u16,
    page_map: *mut Map
}

// PML4
struct Map {
    entries: [Entry; 512]
}

// PDPT and PD
struct Directory {
    entries: [Entry; 512]
}

// PT
struct Table {
    entries: [Entry; 512]
}

struct Entry {
    present: bool,
    write: bool,
    user: bool,
    write_through: bool,
    cache_disable: bool,
    execute_disable: bool,
    entry_type: EntryType
}

enum EntryType {
    Page(Page),
    Pointer(Pointer)
}

struct Page {
    attribute_table: bool,
    global: bool,
    protection_key: u8,
    base: *mut Opaque
}

struct Pointer {
    pointer: *mut Opaque
}

impl PartialEq for Segment {
    fn eq(&self, other: &Segment) -> bool {
        (self.base as usize + self.size > other.base as usize &&
         self.base as usize + self.size < other.base as usize + other.size) ||
            (self.base as usize > other.base as usize + other.size &&
             (self.base as usize) < other.base as usize)
    }
}

impl Eq for Segment {}

impl Ord for Segment {
    fn cmp(&self, other: &Segment) -> Ordering {
        if self == other {
            Ordering::Equal
        } else {
            self.base.cmp(&other.base)
        } 
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Segment) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Segment {
    const fn new(base: *mut Opaque, size: usize, present: bool,
                 write: bool, execute: bool, user: bool) -> Segment {
        Segment {
            base: base,
            size: size,
            present: present,
            write: write,
            execute: execute,
            user: user
        }
    }
}

impl Layout {
    fn new() -> Layout {
        Layout {
            segments: BTreeSet::new()
        }
    }

    fn add_segment(&mut self, segment: Segment) -> bool {
        self.segments.insert(segment)
    }

    fn compile(&self) -> Vec<Frame> {
        let mut frames = vec![];
        let mut base: usize = 0;
        let mut end: usize = 0;

        for segment in self.segments.iter() {
            let alignment: FrameSize;

            // figure out frameset alignment
            if end - base < FrameSize::Big as usize {
                // align to 4k
                alignment = FrameSize::Page;
            } else if end - base < FrameSize::Huge as usize {
                alignment = FrameSize::Big;
            } else {
                alignment = FrameSize::Huge;
            }

            // extend or finish this frameset
            if (segment.base as usize) < align(end, alignment as usize) ||
                segment.base as usize - align(end, alignment as usize) < alignment as usize
            {
                // extend this frameset
                end = segment.base as usize + segment.size;
            } else {
                // push this frameset
                for i in 0..(end + alignment as usize - 1) / alignment as usize {
                    frames.push(Frame {
                        base: (base & !(alignment as usize - 1)) as *mut Opaque, // next lowest
                        size: alignment,
                        present: segment.write,
                        write: segment.write,
                        execute: segment.execute,
                        user: segment.user
                    });
                }

                // set up the next frameset
                base = segment.base as usize;
                end = base + segment.size;
            }
        }

        frames
    }
}

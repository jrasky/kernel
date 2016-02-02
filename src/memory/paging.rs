use memory::Opaque;

// CR3
struct Register {
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
    entry_type: DirectoryType
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

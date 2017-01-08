use collections::{String, Vec};

use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub enum Placement {
    Absolute(u64), // text depends on being loaded at an exact memory address
    Relative { // text depends on being loaded relative to another text
        to: Uuid,
        offset: u64
    },
    Arbitrary(u64) // text does not depend on being loaded at a certain address
    // except maybe some alignment requirements
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Data {
    Offset { partition: u64, offset: u64}, // Text is in a partition, at this offset
    Direct(Vec<u8>), // Text data as a byte vector
    Empty // Explicitly empty text data
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Type {
    Code, // executable (not writable)
    Data { write: bool }, // read, and possibly write data (not executable)
    Info { identity: Uuid } // information, not loaded into memory, with a unique type
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Value {
    Scalar(u64), // a scalar value up to a u64 in size
    Offset(u64), // pointer to a value or method or anything else
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Strategy {
    Identity, // no modification, only guard against overflows
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Chunks {
    Byte, // 8 bits
    Word, // 16 bits
    Long, // 32 bits
    Quad, // 64 bits
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Relocation {
    pub size: Chunks, // how many bytes to fill in
    pub ty: Strategy, // how to compute the value to fill in
    pub offset: u64, // offset into text where to do the relocation
}

// interface bindings

#[derive(Serialize, Deserialize, Debug)]
pub struct Port {
    pub identity: Uuid, // port type
    pub offset: u64, // offset into owning text
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Require {
    pub identity: Uuid, // port type required
    pub places: Vec<Relocation>, // places to fill in this symbol
}

// monolithic bindings

#[derive(Serialize, Deserialize, Debug)]
pub struct Export {
    pub name: String, // name for this symbol
    pub value: Value, // value of this symbol
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Import {
    pub name: String, // name to import
    pub places: Vec<Relocation>, // places to fill in this symbol
}

// Texts are actual code objects

#[derive(Serialize, Deserialize, Debug)]
pub struct Text {
    pub id: u64, // unique ID for this text
    pub base: Placement, // how to place this text in memory
    pub size: u64, // length of this text
    pub ty: Type, // type of this text
    pub provides: Vec<Port>, // ports this text provides
    pub requires: Vec<Require>, // ports this text requires
    pub exports: Vec<Export>, // exported symbols
    pub imports: Vec<Import>, // imported symbols
    pub data: Data, // the data for this text
}

// Modules are linked texts, ready to be used

#[derive(Serialize, Deserialize, Debug)]
pub struct Module {
    pub magic: Uuid, // 0af979b7-02c3-4ca6-b354-b709bec81199
    pub identity: Uuid, // unique ID for this module
    pub size: u64, // the size of this module
    pub partitions: Vec<Partition>, // partitions in this module
    pub texts: Vec<Text> // texts provided by this module
}

// Partitions describe layouts within one byte stream

#[derive(Serialize, Deserialize, Debug)]
pub struct Partition {
    pub index: u64, // the index for this partition
    pub align: u64, // memory alignment of this partition
    pub size: u64 // the size of this partition
}

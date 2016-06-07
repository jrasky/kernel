use uuid::Uuid;

use collections::{String, Vec};

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy)]
pub struct LoadComand {
    offset: u64,
    base: u64,
    size: u64,
    write: bool,
    user: bool,
    execute: bool
}

#[derive(Debug, Clone)]
pub struct Module {
    id: Uuid,
    buffer: RawVec<u8>,
    load_commands: Vec<LoadCommand>
    entry: u64,
}

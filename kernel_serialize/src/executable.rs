use std::fmt;

use uuid::Uuid;

use collections::String;

use alloc::raw_vec::RawVec;

use super::Resource;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ProgramType {
    Text = 0,
    Data = 1
}

pub struct Program {
    id: Uuid,
    variant: ProgramType,
    base: Option<u64>,
    align: u64,
    bytes: Vec<u8>
}

pub struct Symbol {
    id: Uuid,
    name: String,
    version: u64,
    program: Uuid,
    offset: u64,
}

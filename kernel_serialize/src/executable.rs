use std::str::FromStr;

use uuid::Uuid;

use collections::{String, Vec};

use serde::{Serialize, Deserialize};

use super::{Resource, ResourceData};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ProgramType {
    Text = 0,
    Data = 1
}

pub struct Program {
    variant: ProgramType,
    base: Option<u64>,
    align: u64,
    bytes: Vec<u8>
}

pub struct Symbol {
    name: String,
    version: u64,
    program: Resource<Program>,
    offset: u64,
}

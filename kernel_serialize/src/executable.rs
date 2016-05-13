use std::str::FromStr;

use uuid::Uuid;

use collections::{String, Vec};

use super::{Resource, ResourceData};

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
    program: Resource<Program>,
    offset: u64,
}

impl ResourceData for Program {
    fn ty() -> Uuid {
        Uuid::from_str("64ca5221-56e9-413b-8f3d-debf832d5d38").unwrap()
    }

    fn id(&self) -> Option<Uuid> {
        Some(self.id)
    }
}

impl ResourceData for Symbol {
    fn ty() -> Uuid {
        Uuid::from_str("18cf39e6-4507-4108-92cb-4b16f916bfda").unwrap()
    }

    fn id(&self) -> Option<Uuid> {
        Some(self.id)
    }
}

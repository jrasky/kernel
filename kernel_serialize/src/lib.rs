#![no_std]
extern crate core as std;
extern crate constants;
#[macro_use]
extern crate log;
extern crate uuid;
extern crate collections;

use std::any::Any;

use std::fmt;

use uuid::Uuid;

mod executable;

#[derive(Debug, Clone, Copy)]
pub enum ResourceError {
    NoRequest
}

#[derive(Debug, Clone, Copy)]
pub enum SerializerError {
    NotSerializable,
    InvalidData,
    Store(StoreError)
}

pub trait Serializer {
    fn write(&mut self, buf: &[u8]) -> Result<(), SerializerError>;
}

pub trait Deserializer {
    fn read(&mut self, buf: &mut [u8]) -> Result<(), SerializerError>;
}

#[derive(Debug, Clone, Copy)]
pub enum StoreError {
    IdCollision,
    StoreFull,
    ReadOnly,
    Disabled,
    NotFound,
    InUse
}

pub trait Store: Resource {
    fn has(&self, id: Uuid) -> bool;
    fn get(&self, id: Uuid) -> Result<&Resource, StoreError>;
    fn get_mut(&mut self, id: Uuid) -> Result<&mut Resource, StoreError>;
    fn remove(&mut self, id: Uuid) -> Result<Resource, StoreError>;
    fn insert(&mut self, resource: Resource) -> Result<Uuid, StoreError>;
}

pub trait Resource: Any {
    fn ty(&self) -> Uuid;

    fn id(&self) -> Option<Uuid> {
        None
    }

    fn serialize(&self, ser: &mut Serializer) -> Result<(), SerializerError> {
        Err(SerializerError::NotSerializable)
    }

    fn deserialize(id: Uuid, de: &mut Deserializer) -> Result<Self, SerializerError> {
        Err(SerializerError::NotSerializable)
    }
}

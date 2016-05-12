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

pub struct Resource<T: ResourceData> {
    ty: Uuid,
    id: Uuid,
    data: Arc<RefCell<T>>
}

pub trait ResourceData: Any {
    fn serialize(&self, ser: &mut Serializer) -> Result<(), SerializerError> {
        Err(SerializerError::NotSerializable)
    }

    fn deserialize(de: &mut Deserializer) -> Result<Self, SerializerError> {
        Err(SerializerError::NotSerializable)
    }
}

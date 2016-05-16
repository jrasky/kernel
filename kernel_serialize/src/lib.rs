#![feature(custom_derive)]
#![feature(plugin)]
#![feature(collections)]
#![feature(alloc)]
#![feature(coerce_unsized)]
#![feature(raw)]
#![feature(unsize)]
#![plugin(serde_macros)]
#![no_std]
extern crate core as std;
extern crate constants;
#[macro_use]
extern crate log;
extern crate uuid;
extern crate collections;
extern crate alloc;
extern crate serde;
extern crate byteorder;

use std::any::Any;

use std::marker::Unsize;

use std::ops::{CoerceUnsized, Deref};

use std::raw::TraitObject;

use alloc::arc::Arc;

use std::cell::RefCell;

use std::mem;
use std::slice;
use std::char;

use uuid::Uuid;

use serde::{Serialize, Deserialize};

pub use executable::*;

mod executable;

pub struct Resource<T: Data + ?Sized> {
    id: Uuid,
    inner: Option<Arc<RefCell<T>>>
}

pub trait Data: Any + Serialize + Deserialize {
    fn ty() -> Uuid;
}

pub trait Group: Data {
    
}

impl<T: ?Sized + Data> Serialize for Resource {
    fn serialize<S>(&self, serializer: &mut S) -> Result<(), S::Error>
        where S: Serializer {
        if let Some(id) = self.id() {
            id.serialize(serializer)
        } else {
            Err(S::Error::custom("Resource cannot be serialized"))
        }
    }
}

impl<T: Data> Clone for Resource<T> {
    fn clone(&self) -> Self {
        Resource {
            id: self.id,
            inner: self.inner.clone()
        }
    }
}

impl<T: ?Sized + Data> Resource<T> {
    pub fn downcast<U: Data>(self) -> Result<Resource<U>, Self> {
        if Any::is::<Resource<U>>(&self) {
            unsafe {
                let raw: TraitObject = mem::transmute(&self as &Any);
                Ok(mem::transmute(raw.data))
            }
        } else {
            Err(self)
        }
    }
}

impl<T: Data> Resource<T> {
    pub fn new(data: T) -> Resource<T> {
        Resource {
            inner: Arc::new(RefCell::new(data))
        }
    }
}

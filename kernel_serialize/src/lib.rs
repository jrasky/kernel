#![feature(collections)]
#![feature(alloc)]
#![feature(coerce_unsized)]
#![feature(raw)]
#![feature(unsize)]
#![no_std]
extern crate core as std;
extern crate constants;
#[macro_use]
extern crate log;
extern crate uuid;
extern crate collections;
extern crate alloc;

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

pub use executable::*;

mod executable;

pub struct Resource<T: ResourceData + ?Sized> {
    inner: Arc<RefCell<T>>
}

pub trait ResourceData: Any {
    fn ty() -> Uuid;

    fn id(&self) -> Option<Uuid> {
        None
    }
}

impl<T: ?Sized + ResourceData + Unsize<U>, U: ?Sized + ResourceData> CoerceUnsized<Resource<U>> for Resource<T> {}

impl<T: ?Sized + ResourceData> Deref for Resource<T> {
    type Target = RefCell<T>;

    fn deref(&self) -> &RefCell<T> {
        &self.inner
    }
}

impl<T: ResourceData> Clone for Resource<T> {
    fn clone(&self) -> Self {
        Resource {
            inner: self.inner.clone()
        }
    }
}

impl<T: ?Sized + ResourceData> Resource<T> {
    pub fn downcast<U: ResourceData>(self) -> Result<Resource<U>, Self> {
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

impl<T: ResourceData> Resource<T> {
    pub fn new(data: T) -> Resource<T> {
        Resource {
            inner: Arc::new(RefCell::new(data))
        }
    }
}

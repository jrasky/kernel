#[cfg(not(test))]
use core::marker::Reflect;
#[cfg(not(test))]
use core::fmt::{Debug, Display};

#[cfg(test)]
pub use std::error::Error;

#[cfg(not(test))]
pub trait Error: Debug + Display + Reflect {
    fn description(&self) -> &str;

    fn cause(&self) -> Option<&Error> {
        None
    }
}

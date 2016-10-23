pub use std::fmt::{Debug, Display};

#[cfg(test)]
pub use std::error::Error;

#[cfg(not(test))]
pub trait Error: Debug + Display {
    fn description(&self) -> &str;

    fn cause(&self) -> Option<&Error> {
        None
    }
}

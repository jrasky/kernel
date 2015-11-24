use core::marker::Reflect;
use core::fmt::{Debug, Display};

pub trait Error: Debug + Display + Reflect {
    fn description(&self) -> &str;

    fn cause(&self) -> Option<&Error> {
        None
    }
}

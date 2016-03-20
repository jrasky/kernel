#[cfg(not(test))]
pub use core::fmt::{Display, Write};
#[cfg(test)]
pub use std::fmt::{Display, Write};

#[cfg(not(test))]
pub use alloc::boxed::Box;
#[cfg(test)]
pub use std::boxed::Box;

#[cfg(not(test))]
pub use core::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
#[cfg(test)]
pub use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

#[cfg(test)]
pub use std::fmt;
#[cfg(not(test))]
pub use core::fmt;

#[cfg(not(test))]
pub use core::mem;
#[cfg(test)]
pub use std::mem;

pub use collections::{String, Vec};

pub use spin::{RwLock, Mutex};

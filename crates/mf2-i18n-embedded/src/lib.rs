#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod runtime;

pub use crate::runtime::{BasicFormatBackend, EmbeddedPack, EmbeddedRuntime};

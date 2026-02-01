#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod types;

pub use error::{CoreError, CoreResult};
pub use types::{Key, MessageId};

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod args;
mod language_tag;
mod negotiation;
mod types;

pub use args::{ArgType, Args, Value};
pub use error::{CoreError, CoreResult};
pub use language_tag::LanguageTag;
pub use negotiation::{negotiate_lookup, negotiate_lookup_with_trace, NegotiationResult, NegotiationTrace};
pub use types::{Key, MessageId};

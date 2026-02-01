#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod args;
mod bytecode;
mod format_backend;
mod language_tag;
mod negotiation;
mod types;

pub use args::{ArgType, Args, Value};
pub use bytecode::{
    BytecodeProgram, CaseEntry, CaseKey, CaseTable, Opcode, PluralRuleset, StringPool,
};
pub use error::{CoreError, CoreResult};
pub use format_backend::{
    format_value, FormatBackend, FormatterId, FormatterOption, FormatterOptionValue, PluralCategory,
};
pub use language_tag::LanguageTag;
pub use negotiation::{negotiate_lookup, negotiate_lookup_with_trace, NegotiationResult, NegotiationTrace};
pub use types::{Key, MessageId};

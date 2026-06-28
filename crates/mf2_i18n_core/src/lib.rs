#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod args;
mod bytecode;
mod catalog;
mod error;
mod format_backend;
mod interpreter;
mod language_tag;
mod negotiation;
mod pack;
mod pack_catalog;
mod pack_decode;
mod rich_text;
mod types;

pub use args::{ArgType, Args, DateTimeValue, Value};
pub use bytecode::{
    BytecodeProgram, CaseEntry, CaseKey, CaseTable, Opcode, PluralRuleset, StringPool,
};
pub use catalog::{Catalog, CatalogChain};
pub use error::{CoreError, CoreResult};
pub use format_backend::{
    FormatBackend, FormatterId, FormatterOption, FormatterOptionValue, PluralCategory, format_value,
};
pub use interpreter::{execute, execute_rich_text};
pub use language_tag::LanguageTag;
pub use negotiation::{
    NegotiationResult, NegotiationTrace, negotiate_lookup, negotiate_lookup_with_trace,
};
pub use pack::{PackHeader, PackKind, SectionEntry, parse_pack_header, parse_section_directory};
pub use pack_catalog::PackCatalog;
pub use pack_decode::{
    decode_dense_index, decode_sparse_index, decode_string_pool, read_bytecode_at,
};
pub use rich_text::{RichTextMessage, RichTextPart};
pub use types::{I18nMessageKey, Key, MessageId};

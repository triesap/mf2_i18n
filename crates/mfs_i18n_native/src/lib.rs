#![forbid(unsafe_code)]

mod error;
mod localizer;
mod macros;

pub use crate::error::{NativeError, NativeResult};
pub use crate::localizer::{NativeLocalizer, NativeRuntime};
pub use mfs_i18n_core::Args;
pub use mfs_i18n_embedded::EmbeddedPack;
pub use mfs_i18n_std::StdFormatBackend;

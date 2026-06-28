#![forbid(unsafe_code)]

mod error;
mod localizer;
mod macros;

pub use crate::error::{NativeError, NativeResult};
pub use crate::localizer::{NativeLocalizer, NativeRuntime};
pub use mf2_i18n_core::{Args, I18nMessageKey, RichTextMessage, RichTextPart};
pub use mf2_i18n_embedded::EmbeddedPack;
pub use mf2_i18n_std::StdFormatBackend;

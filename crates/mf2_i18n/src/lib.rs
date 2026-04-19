#![forbid(unsafe_code)]

pub use mf2_i18n_core::{
    ArgType, Args, CoreError, CoreResult, DateTimeValue, FormatBackend, FormatterId,
    FormatterOption, FormatterOptionValue, LanguageTag, MessageId, NegotiationResult,
    NegotiationTrace, PackKind, PluralCategory, Value, negotiate_lookup,
    negotiate_lookup_with_trace,
};

#[cfg(feature = "build")]
pub mod build {
    pub use mf2_i18n_build::*;
}

#[cfg(feature = "embedded")]
pub mod embedded {
    pub use mf2_i18n_embedded::*;
}

#[cfg(feature = "native")]
pub mod native {
    pub use mf2_i18n_native::*;
}

#[cfg(feature = "runtime")]
pub mod runtime {
    pub use mf2_i18n_runtime::*;
}

#[cfg(feature = "std_backend")]
pub mod std_backend {
    pub use mf2_i18n_std::*;
}

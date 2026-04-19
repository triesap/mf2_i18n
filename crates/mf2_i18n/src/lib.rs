#![forbid(unsafe_code)]

#[cfg(feature = "build")]
pub use mf2_i18n_build::{
    BuildIoError, CompileError, PlatformBundle, PlatformBundleError, PlatformBundleManifest,
    PlatformPack, ProjectConfig, ProjectError, ProjectLayout, load_platform_bundle_manifest,
    load_project_config, load_project_config_or_default, resolve_config_relative_path,
    write_platform_bundle_manifest,
};
pub use mf2_i18n_core::{
    ArgType, Args, CoreError, CoreResult, DateTimeValue, FormatBackend, FormatterId,
    FormatterOption, FormatterOptionValue, LanguageTag, MessageId, NegotiationResult,
    NegotiationTrace, PackKind, PluralCategory, Value, negotiate_lookup,
    negotiate_lookup_with_trace,
};
#[cfg(feature = "embedded")]
pub use mf2_i18n_embedded::{EmbeddedPack, EmbeddedRuntime};
#[cfg(feature = "native")]
pub use mf2_i18n_native::{
    NativeError, NativeLocalizer, NativeResult, NativeRuntime, define_i18n_module,
};
#[cfg(feature = "runtime")]
pub use mf2_i18n_runtime::{
    IdMap, Manifest, ManifestSigning, PackEntry, Runtime, RuntimeError, RuntimeResult, load_id_map,
    load_manifest, parse_sha256, parse_sha256_literal, verify_manifest_signature,
};
#[cfg(feature = "std_backend")]
pub use mf2_i18n_std::{StdFormatBackend, StdFormatError, StdFormatResolution};

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

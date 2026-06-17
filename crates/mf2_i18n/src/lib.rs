#![forbid(unsafe_code)]

#[cfg(feature = "build")]
pub use mf2_i18n_build::{
    BuildIoError, CompileError, DocusaurusWebBuildError, DocusaurusWebBuildOptions,
    DocusaurusWebBuildOutput, HybridMessageDelivery, HybridMessagePlan, HybridMessageRoute,
    HybridWebAssets, HybridWebBuildError, HybridWebBuildOptions, HybridWebBuildOutput,
    NativeModuleBuildError, NativeModuleBuildOptions, NativeModuleBuildOutput,
    PlainJsonWebBuildError, PlainJsonWebBuildOptions, PlainJsonWebBuildOutput, PlatformBundle,
    PlatformBundleError, PlatformBundleManifest, PlatformPack, ProjectCatalog, ProjectCatalogError,
    ProjectCatalogLoad, ProjectCatalogMessage, ProjectConfig, ProjectError, ProjectLayout,
    ProjectRuntimeBuildOptions, ProjectRuntimeBuildOutput, RuntimeWebAssets, RuntimeWebBuildError,
    RuntimeWebBuildOptions, RuntimeWebBuildOutput, RuntimeWebPackAsset, WEB_ARTIFACT_MANIFEST_FILE,
    WEB_ARTIFACT_MANIFEST_SCHEMA, WebArtifactError, WebArtifactFile, WebArtifactManifest,
    WebArtifactManifestFile, WebArtifactManifestInput, WebArtifactWriteOptions,
    WebArtifactWriteOutput, WebConfig, WebFramework, WebJsonExportError, WebJsonExportOptions,
    WebJsonExportOutput, WebJsonMessageFile, WebJsonMode, WebJsonModeParseError,
    WebJsonUnsupportedKind, WebMessageArg, WebMessageMetadata, WebMessageMetadataError,
    WebStalePolicy, WebTargetConfig, WebTargetConfigError, WebTargetMode,
    build_docusaurus_web_target, build_hybrid_web_target, build_native_module,
    build_plain_json_web_target, build_project_runtime_artifacts, build_runtime_web_target,
    classify_hybrid_messages, collect_web_message_metadata, export_web_json,
    infer_web_message_args, load_platform_bundle_manifest, load_project_catalogs,
    load_project_config, load_project_config_or_default, render_web_typescript_metadata_types,
    resolve_config_relative_path, write_platform_bundle_manifest, write_web_artifacts,
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
    IdMap, Manifest, ManifestSigning, PackEntry, Runtime, RuntimeError, RuntimeParts,
    RuntimeResult, load_id_map, load_manifest, parse_sha256, parse_sha256_literal,
    verify_manifest_signature,
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

#![forbid(unsafe_code)]

pub mod artifacts;
pub mod catalog;
pub mod catalog_builder;
pub mod catalog_reader;
pub mod compiler;
pub mod config;
pub mod diagnostic;
pub mod error;
pub mod extract;
pub mod extract_pipeline;
pub mod id_map;
pub mod lexer;
pub mod locale_sources;
pub mod manifest;
pub mod mf2_source;
pub mod micro_locales;
pub mod model;
pub mod pack_encode;
pub mod parser;
pub mod platform;
pub mod project;
pub mod project_catalogs;
pub mod project_pipeline;
pub mod validator;
pub mod web;
pub mod web_docusaurus;
pub mod web_hybrid;
pub mod web_json;
pub mod web_metadata;
pub mod web_runtime;

pub use crate::compiler::CompileError;
pub use crate::config::{
    PROJECT_CONFIG_VERSION, ProjectConfig, ProjectConfigError, ProjectTargetConfig, SourceConfig,
    SourceLayout, WebConfig, WebFramework, WebPreset, WebStalePolicy, WebTargetConfig,
    WebTargetConfigError, WebTargetMode, load_project_config, load_project_config_or_default,
};
pub use crate::error::BuildIoError;
pub use crate::platform::{
    PlatformBundle, PlatformBundleError, PlatformBundleManifest, PlatformPack,
    load_platform_bundle_manifest, write_platform_bundle_manifest,
};
pub use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
pub use crate::project_catalogs::{
    ProjectCatalog, ProjectCatalogError, ProjectCatalogLoad, ProjectCatalogMessage,
    load_project_catalogs,
};
pub use crate::project_pipeline::{
    NativeModuleBuildError, NativeModuleBuildOptions, NativeModuleBuildOutput,
    ProjectRuntimeBuildOptions, ProjectRuntimeBuildOutput, build_native_module,
    build_project_runtime_artifacts,
};
pub use crate::web::{
    WEB_ARTIFACT_MANIFEST_FILE, WEB_ARTIFACT_MANIFEST_SCHEMA, WebArtifactError, WebArtifactFile,
    WebArtifactManifest, WebArtifactManifestFile, WebArtifactManifestInput,
    WebArtifactWriteOptions, WebArtifactWriteOutput, write_web_artifacts,
};
pub use crate::web_docusaurus::{
    DocusaurusWebBuildError, DocusaurusWebBuildOptions, DocusaurusWebBuildOutput,
    build_docusaurus_web_target,
};
pub use crate::web_hybrid::{
    HybridWebAssets, HybridWebBuildError, HybridWebBuildOptions, HybridWebBuildOutput,
    build_hybrid_web_target,
};
pub use crate::web_json::{
    PlainJsonWebBuildError, PlainJsonWebBuildOptions, PlainJsonWebBuildOutput, WebJsonExportError,
    WebJsonExportOptions, WebJsonExportOutput, WebJsonMessageFile, WebJsonMode,
    WebJsonModeParseError, WebJsonUnsupportedKind, build_plain_json_web_target, export_web_json,
};
pub use crate::web_metadata::{
    HybridMessageDelivery, HybridMessagePlan, HybridMessageRoute, WebMessageArg,
    WebMessageMetadata, WebMessageMetadataError, classify_hybrid_messages,
    collect_web_message_metadata, infer_web_message_args, render_web_typescript_metadata_types,
};
pub use crate::web_runtime::{
    RuntimeWebAssets, RuntimeWebBuildError, RuntimeWebBuildOptions, RuntimeWebBuildOutput,
    RuntimeWebPackAsset, build_runtime_web_target,
};

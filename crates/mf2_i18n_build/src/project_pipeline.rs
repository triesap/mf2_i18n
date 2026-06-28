use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use thiserror::Error;

use crate::artifacts::{write_id_map, write_id_map_hash};
use crate::compiler::{CompileError, compile_message};
use crate::generated_keys::{
    GeneratedKeyError, GeneratedMessageKey, build_generated_message_keys, rust_value_identifier,
    variant_function_suffix,
};
use crate::id_map::{IdMap, IdMapError, build_id_map};
use crate::manifest::{Manifest, PackEntry, sha256_hex};
use crate::message_metadata::{
    MessageArg, MessageMetadata, MessageMetadataError, collect_message_metadata,
};
use crate::micro_locales::{MicroLocaleError, load_micro_locales};
use crate::model::ArgType;
use crate::pack_encode::{PackBuildInput, encode_pack};
use crate::parser::parse_message;
use crate::platform::{PlatformBundleManifest, write_platform_bundle_manifest};
use crate::project::{ProjectError, ProjectLayout};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogError, load_project_catalogs};

const DEFAULT_MODULE_MACRO_PATH: &str = "mf2_i18n::define_i18n_module!";
const DEFAULT_GENERATED_AT: &str = "1970-01-01T00:00:00Z";
const GENERATED_MODULE_FILE: &str = "generated_module.rs";
const GENERATED_CATALOG_FILE: &str = "generated_catalog.rs";
const MANIFEST_FILE: &str = "manifest.json";
const PACKS_DIR: &str = "packs";
const PLATFORM_BUNDLE_FILE: &str = "platform-bundle.json";
const ID_MAP_JSON_FILE: &str = "id-map.json";
const ID_MAP_HASH_FILE: &str = "id-map.sha256";

type Catalog = ProjectCatalog;

#[derive(Debug, Clone)]
pub struct ProjectRuntimeBuildOptions {
    config_path: PathBuf,
    out_dir: PathBuf,
    release_id: String,
    generated_at: String,
    locale_filter: Vec<String>,
    namespace_filter: Vec<String>,
}

impl ProjectRuntimeBuildOptions {
    pub fn new(
        config_path: impl Into<PathBuf>,
        out_dir: impl Into<PathBuf>,
        release_id: impl Into<String>,
        generated_at: impl Into<String>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            out_dir: out_dir.into(),
            release_id: release_id.into(),
            generated_at: generated_at.into(),
            locale_filter: Vec::new(),
            namespace_filter: Vec::new(),
        }
    }

    pub fn with_locales<I, S>(mut self, locales: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.locale_filter = locales.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_namespaces<I, S>(mut self, namespaces: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.namespace_filter = namespaces.into_iter().map(Into::into).collect();
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    pub fn generated_at(&self) -> &str {
        &self.generated_at
    }

    pub fn locale_filter(&self) -> &[String] {
        &self.locale_filter
    }

    pub fn namespace_filter(&self) -> &[String] {
        &self.namespace_filter
    }
}

#[derive(Debug, Clone)]
pub struct NativeModuleBuildOptions {
    config_path: PathBuf,
    out_dir: PathBuf,
    artifact_dir_name: String,
    release_id: String,
    generated_at: String,
    module_macro_path: String,
}

impl NativeModuleBuildOptions {
    pub fn new(
        config_path: impl Into<PathBuf>,
        out_dir: impl Into<PathBuf>,
        artifact_dir_name: impl Into<String>,
    ) -> Self {
        let artifact_dir_name = artifact_dir_name.into();
        Self {
            config_path: config_path.into(),
            out_dir: out_dir.into(),
            release_id: artifact_dir_name.clone(),
            artifact_dir_name,
            generated_at: DEFAULT_GENERATED_AT.to_owned(),
            module_macro_path: DEFAULT_MODULE_MACRO_PATH.to_owned(),
        }
    }

    pub fn with_release_metadata(
        mut self,
        release_id: impl Into<String>,
        generated_at: impl Into<String>,
    ) -> Self {
        self.release_id = release_id.into();
        self.generated_at = generated_at.into();
        self
    }

    pub fn with_module_macro_path(mut self, module_macro_path: impl Into<String>) -> Self {
        self.module_macro_path = module_macro_path.into();
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn artifact_dir_name(&self) -> &str {
        &self.artifact_dir_name
    }

    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    pub fn generated_at(&self) -> &str {
        &self.generated_at
    }

    pub fn module_macro_path(&self) -> &str {
        &self.module_macro_path
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRuntimeBuildOutput {
    artifact_dir: PathBuf,
    id_map_path: PathBuf,
    id_map_hash_path: PathBuf,
    packs_dir: PathBuf,
    manifest_path: PathBuf,
    platform_bundle_path: PathBuf,
    rerun_if_changed_paths: Vec<PathBuf>,
    default_locale: String,
    supported_locales: Vec<String>,
    default_catalog_keys: Vec<String>,
    default_message_metadata: Vec<MessageMetadata>,
}

impl ProjectRuntimeBuildOutput {
    pub fn artifact_dir(&self) -> &Path {
        &self.artifact_dir
    }

    pub fn id_map_path(&self) -> &Path {
        &self.id_map_path
    }

    pub fn id_map_hash_path(&self) -> &Path {
        &self.id_map_hash_path
    }

    pub fn packs_dir(&self) -> &Path {
        &self.packs_dir
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn platform_bundle_path(&self) -> &Path {
        &self.platform_bundle_path
    }

    pub fn rerun_if_changed_paths(&self) -> &[PathBuf] {
        &self.rerun_if_changed_paths
    }

    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn supported_locales(&self) -> &[String] {
        &self.supported_locales
    }

    pub fn default_catalog_keys(&self) -> &[String] {
        &self.default_catalog_keys
    }

    pub fn default_message_metadata(&self) -> &[MessageMetadata] {
        &self.default_message_metadata
    }
}

#[derive(Debug, Clone)]
pub struct NativeModuleBuildOutput {
    runtime_output: ProjectRuntimeBuildOutput,
    generated_module_path: PathBuf,
    generated_catalog_path: PathBuf,
}

impl NativeModuleBuildOutput {
    pub fn artifact_dir(&self) -> &Path {
        self.runtime_output.artifact_dir()
    }

    pub fn id_map_path(&self) -> &Path {
        self.runtime_output.id_map_path()
    }

    pub fn id_map_hash_path(&self) -> &Path {
        self.runtime_output.id_map_hash_path()
    }

    pub fn packs_dir(&self) -> &Path {
        self.runtime_output.packs_dir()
    }

    pub fn manifest_path(&self) -> &Path {
        self.runtime_output.manifest_path()
    }

    pub fn platform_bundle_path(&self) -> &Path {
        self.runtime_output.platform_bundle_path()
    }

    pub fn generated_module_path(&self) -> &Path {
        &self.generated_module_path
    }

    pub fn generated_catalog_path(&self) -> &Path {
        &self.generated_catalog_path
    }

    pub fn rerun_if_changed_paths(&self) -> &[PathBuf] {
        self.runtime_output.rerun_if_changed_paths()
    }

    pub fn default_locale(&self) -> &str {
        self.runtime_output.default_locale()
    }

    pub fn supported_locales(&self) -> &[String] {
        self.runtime_output.supported_locales()
    }

    pub fn default_catalog_keys(&self) -> &[String] {
        self.runtime_output.default_catalog_keys()
    }

    pub fn default_message_metadata(&self) -> &[MessageMetadata] {
        self.runtime_output.default_message_metadata()
    }
}

#[derive(Debug, Error)]
pub enum NativeModuleBuildError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Catalogs(#[from] ProjectCatalogError),
    #[error(transparent)]
    IdMap(#[from] IdMapError),
    #[error(transparent)]
    GeneratedKeys(#[from] GeneratedKeyError),
    #[error(transparent)]
    MessageMetadata(#[from] MessageMetadataError),
    #[error(transparent)]
    MicroLocales(#[from] MicroLocaleError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("artifact dir name must not be empty")]
    EmptyArtifactDirName,
    #[error("artifact dir name must be a single relative path segment: {0}")]
    InvalidArtifactDirName(String),
    #[error("release id must not be empty")]
    EmptyReleaseId,
    #[error("generated_at must not be empty")]
    EmptyGeneratedAt,
    #[error("module macro path must not be empty")]
    EmptyModuleMacroPath,
    #[error(
        "failed to parse i18n message for locale {locale} key {key}: {message} at {line}:{column}"
    )]
    Parse {
        locale: String,
        key: String,
        message: String,
        line: u32,
        column: u32,
    },
    #[error("failed to compile i18n message for locale {locale} key {key}: {source}")]
    Compile {
        locale: String,
        key: String,
        #[source]
        source: CompileError,
    },
    #[error("missing message id for locale {locale} key {key}")]
    MissingMessageId { locale: String, key: String },
}

pub fn build_project_runtime_artifacts(
    options: &ProjectRuntimeBuildOptions,
) -> Result<ProjectRuntimeBuildOutput, NativeModuleBuildError> {
    validate_runtime_options(options.release_id(), options.generated_at())?;

    let project = ProjectLayout::load_or_default(options.config_path())?;
    let mut rerun_paths = BTreeSet::from([options.config_path().to_path_buf()]);
    rerun_paths.insert(project.project_salt_path());
    let id_salt = project.load_project_salt()?;
    let loaded_catalogs = load_project_catalogs(&project)?;
    rerun_paths.extend(loaded_catalogs.rerun_if_changed_paths().iter().cloned());
    let catalogs = filter_runtime_catalogs(
        loaded_catalogs.catalogs(),
        options.locale_filter(),
        options.namespace_filter(),
    );
    let micro_locale_map = project
        .micro_locales_registry_path()
        .map(|path| {
            rerun_paths.insert(path.clone());
            load_micro_locales(&path)
        })
        .transpose()?
        .unwrap_or_default();

    let default_locale = project.config().default_locale.clone();
    let default_catalog = catalogs
        .get(&default_locale)
        .ok_or_else(|| ProjectCatalogError::MissingDefaultLocale(default_locale.clone()))?;
    let default_catalog_keys = default_catalog.keys().cloned().collect::<Vec<_>>();
    let default_message_metadata =
        collect_message_metadata(&default_locale, default_catalog, None)?;

    let id_map = build_id_map(default_catalog_keys.iter().cloned(), &id_salt)?;
    let id_map_hash = id_map.hash()?;

    let artifact_dir = options.out_dir().to_path_buf();
    fs::create_dir_all(&artifact_dir)?;

    let id_map_path = artifact_dir.join(ID_MAP_JSON_FILE);
    let id_map_hash_path = artifact_dir.join(ID_MAP_HASH_FILE);
    write_id_map(&id_map_path, &id_map)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    write_id_map_hash(&id_map_hash_path, id_map_hash)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let supported_locales = catalogs.keys().cloned().collect::<Vec<_>>();
    let packs_dir = artifact_dir.join(PACKS_DIR);
    fs::create_dir_all(&packs_dir)?;
    let mut mf2_packs = BTreeMap::new();
    let mut micro_locales = BTreeMap::new();
    for (locale, catalog) in &catalogs {
        let parent = micro_locale_map.get(locale).cloned();
        let pack_kind = if parent.is_some() {
            mf2_i18n_core::PackKind::Overlay
        } else {
            mf2_i18n_core::PackKind::Base
        };
        let pack_bytes = compile_catalog_pack(
            locale,
            catalog,
            &id_map,
            id_map_hash,
            pack_kind,
            parent.as_deref(),
        )?;
        let filename = format!("{locale}.mf2pack");
        let pack_path = packs_dir.join(&filename);
        fs::write(&pack_path, &pack_bytes)?;
        if let Some(parent) = &parent {
            micro_locales.insert(locale.clone(), parent.clone());
        }
        mf2_packs.insert(
            locale.clone(),
            PackEntry {
                kind: match pack_kind {
                    mf2_i18n_core::PackKind::Base => "base".to_owned(),
                    mf2_i18n_core::PackKind::Overlay => "overlay".to_owned(),
                    mf2_i18n_core::PackKind::IcuData => "icu_data".to_owned(),
                },
                url: format!("{PACKS_DIR}/{filename}"),
                hash: sha256_hex(&pack_bytes),
                size: pack_bytes.len() as u64,
                content_encoding: "identity".to_owned(),
                pack_schema: 0,
                parent,
            },
        );
    }

    let manifest = Manifest {
        schema: 1,
        release_id: options.release_id().to_owned(),
        generated_at: options.generated_at().to_owned(),
        default_locale: default_locale.clone(),
        supported_locales: supported_locales.clone(),
        id_map_hash: format!("sha256:{}", hex::encode(id_map_hash)),
        mf2_packs,
        icu_packs: None,
        micro_locales: (!micro_locales.is_empty()).then_some(micro_locales),
        budgets: None,
        signing: None,
    };
    let manifest_path = artifact_dir.join(MANIFEST_FILE);
    fs::write(&manifest_path, manifest.to_canonical_bytes())?;

    let platform_bundle = PlatformBundleManifest::new(manifest, ID_MAP_JSON_FILE);
    let platform_bundle_path = artifact_dir.join(PLATFORM_BUNDLE_FILE);
    write_platform_bundle_manifest(&platform_bundle_path, &platform_bundle)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    Ok(ProjectRuntimeBuildOutput {
        artifact_dir,
        id_map_path,
        id_map_hash_path,
        packs_dir,
        manifest_path,
        platform_bundle_path,
        rerun_if_changed_paths: rerun_paths.into_iter().collect(),
        default_locale,
        supported_locales,
        default_catalog_keys,
        default_message_metadata,
    })
}

fn filter_runtime_catalogs(
    catalogs: &BTreeMap<String, ProjectCatalog>,
    locale_filter: &[String],
    namespace_filter: &[String],
) -> BTreeMap<String, ProjectCatalog> {
    let mut filtered = BTreeMap::new();
    for (locale, catalog) in catalogs {
        if !locale_filter.is_empty() && !locale_filter.contains(locale) {
            continue;
        }
        let messages = catalog
            .iter()
            .filter(|(_, message)| {
                namespace_filter.is_empty() || namespace_filter.contains(&message.namespace)
            })
            .map(|(key, message)| (key.clone(), message.clone()))
            .collect::<ProjectCatalog>();
        filtered.insert(locale.clone(), messages);
    }
    filtered
}

pub fn build_native_module(
    options: &NativeModuleBuildOptions,
) -> Result<NativeModuleBuildOutput, NativeModuleBuildError> {
    validate_options(options)?;

    let runtime_output = build_project_runtime_artifacts(&ProjectRuntimeBuildOptions::new(
        options.config_path(),
        options.out_dir().join(options.artifact_dir_name()),
        options.release_id(),
        options.generated_at(),
    ))?;

    let generated_message_keys =
        build_generated_message_keys(runtime_output.default_catalog_keys())?;

    let generated_module_path = runtime_output.artifact_dir().join(GENERATED_MODULE_FILE);
    fs::write(
        &generated_module_path,
        render_generated_module(
            runtime_output.default_locale(),
            runtime_output.supported_locales(),
            &generated_message_keys,
            runtime_output.default_message_metadata(),
            options.artifact_dir_name(),
            options.module_macro_path(),
        )?,
    )?;

    let generated_catalog_path = runtime_output.artifact_dir().join(GENERATED_CATALOG_FILE);
    fs::write(
        &generated_catalog_path,
        render_generated_catalog(
            runtime_output.default_locale(),
            runtime_output.supported_locales(),
            runtime_output.default_catalog_keys(),
            &generated_message_keys,
        ),
    )?;

    Ok(NativeModuleBuildOutput {
        runtime_output,
        generated_module_path,
        generated_catalog_path,
    })
}

fn validate_options(options: &NativeModuleBuildOptions) -> Result<(), NativeModuleBuildError> {
    if options.artifact_dir_name().trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyArtifactDirName);
    }

    validate_runtime_options(options.release_id(), options.generated_at())?;

    let components = Path::new(options.artifact_dir_name())
        .components()
        .collect::<Vec<_>>();
    if components.len() != 1 || !matches!(components.first(), Some(Component::Normal(_))) {
        return Err(NativeModuleBuildError::InvalidArtifactDirName(
            options.artifact_dir_name().to_owned(),
        ));
    }

    if options.module_macro_path().trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyModuleMacroPath);
    }

    Ok(())
}

fn validate_runtime_options(
    release_id: &str,
    generated_at: &str,
) -> Result<(), NativeModuleBuildError> {
    if release_id.trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyReleaseId);
    }

    if generated_at.trim().is_empty() {
        return Err(NativeModuleBuildError::EmptyGeneratedAt);
    }

    Ok(())
}

fn compile_catalog_pack(
    locale: &str,
    catalog: &Catalog,
    id_map: &IdMap,
    id_map_hash: [u8; 32],
    pack_kind: mf2_i18n_core::PackKind,
    parent_tag: Option<&str>,
) -> Result<Vec<u8>, NativeModuleBuildError> {
    let mut messages = BTreeMap::new();

    for (key, message) in catalog {
        let parsed =
            parse_message(&message.value).map_err(|error| NativeModuleBuildError::Parse {
                locale: locale.to_owned(),
                key: key.clone(),
                message: error.message,
                line: error.span.line,
                column: error.span.column,
            })?;
        let compiled =
            compile_message(&parsed).map_err(|source| NativeModuleBuildError::Compile {
                locale: locale.to_owned(),
                key: key.clone(),
                source,
            })?;
        let message_id =
            id_map
                .get(key)
                .ok_or_else(|| NativeModuleBuildError::MissingMessageId {
                    locale: locale.to_owned(),
                    key: key.clone(),
                })?;
        messages.insert(message_id, compiled.program);
    }

    Ok(encode_pack(&PackBuildInput {
        pack_kind,
        id_map_hash,
        locale_tag: locale.to_owned(),
        parent_tag: parent_tag.map(str::to_owned),
        build_epoch_ms: 0,
        messages,
    }))
}

fn render_generated_module(
    default_locale: &str,
    supported_locales: &[String],
    generated_message_keys: &[GeneratedMessageKey],
    message_metadata: &[MessageMetadata],
    artifact_dir_name: &str,
    module_macro_path: &str,
) -> Result<String, NativeModuleBuildError> {
    let packs_source = supported_locales
        .iter()
        .map(|locale| {
            format!(
                "            ({locale:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{PACKS_DIR}/{locale}.mf2pack\"))),"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let supported_locale_values = supported_locales
        .iter()
        .map(|locale| format!("{locale:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let key_variants = generated_message_keys
        .iter()
        .map(|key| format!("    {},", key.variant))
        .collect::<Vec<_>>()
        .join("\n");
    let all_key_values = generated_message_keys
        .iter()
        .map(|key| format!("        Self::{},", key.variant))
        .collect::<Vec<_>>()
        .join("\n");
    let key_id_matches = generated_message_keys
        .iter()
        .map(|key| format!("            Self::{} => {:?},", key.variant, key.id))
        .collect::<Vec<_>>()
        .join("\n");
    let metadata_by_id = message_metadata
        .iter()
        .map(|metadata| (metadata.id.as_str(), metadata))
        .collect::<BTreeMap<_, _>>();
    let mut typed_args_source = String::new();
    let mut typed_helpers_source = String::new();
    for key in generated_message_keys {
        let metadata = metadata_by_id.get(key.id.as_str()).copied();
        let args = metadata
            .map(|value| value.args.as_slice())
            .unwrap_or_default();
        if !args.is_empty() {
            typed_args_source.push_str(&render_generated_args_struct(key, args)?);
        }
        typed_helpers_source.push_str(&render_generated_typed_helpers(key, args)?);
    }

    Ok(format!(
        "mod generated {{\n    {module_macro_path} {{\n        init_policy: strict,\n        default_locale: {default_locale:?},\n        id_map_json: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{ID_MAP_JSON_FILE}\")),\n        id_map_hash: include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{artifact_dir_name}/{ID_MAP_HASH_FILE}\")),\n        packs: [\n{packs_source}\n        ],\n    }}\n}}\n\n#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]\npub enum MessageKey {{\n{key_variants}\n}}\n\nimpl MessageKey {{\n    pub const ALL: &'static [Self] = &[\n{all_key_values}\n    ];\n\n    pub const fn id(self) -> &'static str {{\n        match self {{\n{key_id_matches}\n        }}\n    }}\n}}\n\nimpl ::core::fmt::Display for MessageKey {{\n    fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {{\n        formatter.write_str(self.id())\n    }}\n}}\n\nimpl mf2_i18n::I18nMessageKey for MessageKey {{\n    fn id(self) -> &'static str {{\n        MessageKey::id(self)\n    }}\n}}\n\npub const DEFAULT_LOCALE: &str = {default_locale:?};\npub const SUPPORTED_LOCALES: &[&str] = &[{supported_locale_values}];\npub const DEFAULT_CATALOG_KEYS: &[MessageKey] = MessageKey::ALL;\n\npub fn localizer() -> &'static mf2_i18n::NativeLocalizer {{\n    generated::localizer()\n}}\n\npub fn set_preferred_locales(preferred_locales: &[&str]) -> mf2_i18n::NativeResult<String> {{\n    generated::set_preferred_locales(preferred_locales)\n}}\n\npub fn set_locale(locale: &str) -> mf2_i18n::NativeResult<String> {{\n    generated::set_locale(locale)\n}}\n\npub fn negotiate_locale(preferred_locales: &[&str]) -> mf2_i18n::NativeResult<String> {{\n    generated::negotiate_locale(preferred_locales)\n}}\n\npub fn locale() -> String {{\n    generated::locale()\n}}\n\npub fn try_t(key: MessageKey) -> mf2_i18n::NativeResult<String> {{\n    generated::tr(key.id())\n}}\n\npub fn t(key: MessageKey) -> String {{\n    try_t(key).expect(\"mf2_i18n generated message formatting failed\")\n}}\n\npub fn t_or_key(key: MessageKey) -> String {{\n    generated::tr_or_key(key.id())\n}}\n\npub fn try_t_locale(locale: &str, key: MessageKey) -> mf2_i18n::NativeResult<String> {{\n    generated::tr_locale(locale, key.id())\n}}\n\npub fn t_locale(locale: &str, key: MessageKey) -> String {{\n    try_t_locale(locale, key).expect(\"mf2_i18n generated message formatting failed\")\n}}\n\npub fn t_locale_or_key(locale: &str, key: MessageKey) -> String {{\n    generated::tr_locale_or_key(locale, key.id())\n}}\n\npub fn try_t_with_args(key: MessageKey, args: &mf2_i18n::Args) -> mf2_i18n::NativeResult<String> {{\n    generated::tr_with_args(key.id(), args)\n}}\n\npub fn t_with_args(key: MessageKey, args: &mf2_i18n::Args) -> String {{\n    try_t_with_args(key, args).expect(\"mf2_i18n generated message formatting failed\")\n}}\n\npub fn t_with_args_or_key(key: MessageKey, args: &mf2_i18n::Args) -> String {{\n    generated::tr_with_args_or_key(key.id(), args)\n}}\n\npub fn try_t_with_args_locale(locale: &str, key: MessageKey, args: &mf2_i18n::Args) -> mf2_i18n::NativeResult<String> {{\n    generated::tr_with_args_locale(locale, key.id(), args)\n}}\n\npub fn t_with_args_locale(locale: &str, key: MessageKey, args: &mf2_i18n::Args) -> String {{\n    try_t_with_args_locale(locale, key, args).expect(\"mf2_i18n generated message formatting failed\")\n}}\n\npub fn t_with_args_locale_or_key(locale: &str, key: MessageKey, args: &mf2_i18n::Args) -> String {{\n    generated::tr_with_args_locale_or_key(locale, key.id(), args)\n}}\n"
    ) + &typed_args_source
        + &typed_helpers_source)
}

struct GeneratedArgParam {
    name: String,
    ty: String,
}

fn render_generated_args_struct(
    key: &GeneratedMessageKey,
    args: &[MessageArg],
) -> Result<String, NativeModuleBuildError> {
    let struct_name = generated_args_struct_name(key);
    let params = generated_arg_params(args)?;
    let param_values = params
        .iter()
        .map(|param| format!("{}: {}", param.name, param.ty))
        .collect::<Vec<_>>()
        .join(", ");
    let insertions = args
        .iter()
        .map(render_arg_insertion)
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");

    Ok(format!(
        "\npub struct {struct_name} {{\n    args: mf2_i18n::Args,\n}}\n\nimpl {struct_name} {{\n    pub fn new({param_values}) -> Self {{\n        let mut args = mf2_i18n::Args::new();\n{insertions}\n        Self {{ args }}\n    }}\n\n    pub fn as_args(&self) -> &mf2_i18n::Args {{\n        &self.args\n    }}\n\n    pub fn into_args(self) -> mf2_i18n::Args {{\n        self.args\n    }}\n}}\n\nimpl ::core::convert::AsRef<mf2_i18n::Args> for {struct_name} {{\n    fn as_ref(&self) -> &mf2_i18n::Args {{\n        self.as_args()\n    }}\n}}\n"
    ))
}

fn render_generated_typed_helpers(
    key: &GeneratedMessageKey,
    args: &[MessageArg],
) -> Result<String, NativeModuleBuildError> {
    let suffix = variant_function_suffix(&key.variant);
    let variant = &key.variant;
    if args.is_empty() {
        return Ok(format!(
            "\npub fn try_t_{suffix}() -> mf2_i18n::NativeResult<String> {{\n    try_t(MessageKey::{variant})\n}}\n\npub fn t_{suffix}() -> String {{\n    t(MessageKey::{variant})\n}}\n\npub fn t_{suffix}_or_key() -> String {{\n    t_or_key(MessageKey::{variant})\n}}\n"
        ));
    }

    let struct_name = generated_args_struct_name(key);
    Ok(format!(
        "\npub fn try_t_{suffix}(args: &{struct_name}) -> mf2_i18n::NativeResult<String> {{\n    try_t_with_args(MessageKey::{variant}, args.as_args())\n}}\n\npub fn t_{suffix}(args: &{struct_name}) -> String {{\n    t_with_args(MessageKey::{variant}, args.as_args())\n}}\n\npub fn t_{suffix}_or_key(args: &{struct_name}) -> String {{\n    t_with_args_or_key(MessageKey::{variant}, args.as_args())\n}}\n"
    ))
}

fn generated_args_struct_name(key: &GeneratedMessageKey) -> String {
    format!("{}Args", key.variant)
}

fn generated_arg_params(
    args: &[MessageArg],
) -> Result<Vec<GeneratedArgParam>, NativeModuleBuildError> {
    let mut used = BTreeMap::<String, String>::new();
    let mut params = Vec::new();
    for arg in args {
        match arg.arg_type {
            ArgType::String => push_arg_param(
                &mut params,
                &mut used,
                rust_value_identifier(&arg.name)?,
                "impl Into<String>".to_string(),
                arg.name.clone(),
            )?,
            ArgType::Number => push_arg_param(
                &mut params,
                &mut used,
                rust_value_identifier(&arg.name)?,
                "f64".to_string(),
                arg.name.clone(),
            )?,
            ArgType::Bool => push_arg_param(
                &mut params,
                &mut used,
                rust_value_identifier(&arg.name)?,
                "bool".to_string(),
                arg.name.clone(),
            )?,
            ArgType::DateTime => push_arg_param(
                &mut params,
                &mut used,
                rust_value_identifier(&arg.name)?,
                "mf2_i18n::DateTimeValue".to_string(),
                arg.name.clone(),
            )?,
            ArgType::Unit => {
                let base = rust_value_identifier(&arg.name)?;
                push_arg_param(
                    &mut params,
                    &mut used,
                    format!("{base}_value"),
                    "f64".to_string(),
                    format!("{}.value", arg.name),
                )?;
                push_arg_param(
                    &mut params,
                    &mut used,
                    format!("{base}_unit_id"),
                    "u32".to_string(),
                    format!("{}.unit_id", arg.name),
                )?;
            }
            ArgType::Currency => {
                let base = rust_value_identifier(&arg.name)?;
                push_arg_param(
                    &mut params,
                    &mut used,
                    format!("{base}_value"),
                    "f64".to_string(),
                    format!("{}.value", arg.name),
                )?;
                push_arg_param(
                    &mut params,
                    &mut used,
                    format!("{base}_code"),
                    "[u8; 3]".to_string(),
                    format!("{}.code", arg.name),
                )?;
            }
            ArgType::Any => push_arg_param(
                &mut params,
                &mut used,
                rust_value_identifier(&arg.name)?,
                "mf2_i18n::Value".to_string(),
                arg.name.clone(),
            )?,
        }
    }
    Ok(params)
}

fn push_arg_param(
    params: &mut Vec<GeneratedArgParam>,
    used: &mut BTreeMap<String, String>,
    name: String,
    ty: String,
    source: String,
) -> Result<(), NativeModuleBuildError> {
    if let Some(first_value) = used.insert(name.clone(), source.clone()) {
        return Err(GeneratedKeyError::IdentifierCollision {
            identifier: name,
            first_value,
            duplicate_value: source,
        }
        .into());
    }
    params.push(GeneratedArgParam { name, ty });
    Ok(())
}

fn render_arg_insertion(arg: &MessageArg) -> Result<String, NativeModuleBuildError> {
    let name = rust_value_identifier(&arg.name)?;
    let key = &arg.name;
    Ok(match arg.arg_type {
        ArgType::String => {
            format!("        args.insert({key:?}, mf2_i18n::Value::Str({name}.into()));")
        }
        ArgType::Number => format!("        args.insert({key:?}, mf2_i18n::Value::Num({name}));"),
        ArgType::Bool => format!("        args.insert({key:?}, mf2_i18n::Value::Bool({name}));"),
        ArgType::DateTime => {
            format!("        args.insert({key:?}, mf2_i18n::Value::DateTime({name}));")
        }
        ArgType::Unit => format!(
            "        args.insert({key:?}, mf2_i18n::Value::Unit {{ value: {name}_value, unit_id: {name}_unit_id }});"
        ),
        ArgType::Currency => format!(
            "        args.insert({key:?}, mf2_i18n::Value::Currency {{ value: {name}_value, code: {name}_code }});"
        ),
        ArgType::Any => format!("        args.insert({key:?}, {name});"),
    })
}

fn render_generated_catalog(
    default_locale: &str,
    supported_locales: &[String],
    default_catalog_keys: &[String],
    generated_message_keys: &[GeneratedMessageKey],
) -> String {
    let supported_locale_values = supported_locales
        .iter()
        .map(|locale| format!("{locale:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let key_values = default_catalog_keys
        .iter()
        .map(|key| format!("{key:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let key_variant_values = generated_message_keys
        .iter()
        .map(|key| format!("({:?}, {:?})", key.id, key.variant))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "const DEFAULT_LOCALE_ID: &str = {default_locale:?};\nconst SUPPORTED_LOCALE_IDS: &[&str] = &[{supported_locale_values}];\n#[cfg(test)] const DEFAULT_CATALOG_KEY_IDS: &[&str] = &[{key_values}];\n#[cfg(test)] const DEFAULT_CATALOG_KEY_VARIANTS: &[(&str, &str)] = &[{key_variant_values}];\n"
    )
}

#[cfg(test)]
mod tests {
    use super::{
        GENERATED_CATALOG_FILE, NativeModuleBuildError, NativeModuleBuildOptions,
        ProjectRuntimeBuildOptions, build_native_module, build_project_runtime_artifacts,
        render_generated_args_struct,
    };
    use crate::generated_keys::{GeneratedKeyError, GeneratedMessageKey};
    use crate::message_metadata::MessageArg;
    use crate::model::ArgType;
    use crate::platform::PlatformBundle;
    use crate::project_catalogs::ProjectCatalogError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_native_module_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_catalog(path: &Path, messages: &[(&str, &str)]) {
        let contents = messages
            .iter()
            .map(|(key, value)| format!("  {key:?}: {value:?}"))
            .collect::<Vec<_>>()
            .join(",\n");
        fs::write(path, format!("{{\n{contents}\n}}\n")).expect("write");
    }

    fn write_project_fixture(root: &Path) -> PathBuf {
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let french_dir = locales_root.join("fr");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&french_dir).expect("french locale dir");

        write_catalog(
            &english_dir.join("common.json"),
            &[
                ("home.title", "Hi { $name }"),
                ("home.subtitle", "Grow from the root"),
            ],
        );
        write_catalog(
            &french_dir.join("common.json"),
            &[
                ("home.title", "Salut { $name }"),
                ("home.subtitle", "Cultiver depuis la racine"),
            ],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");
        config_path
    }

    fn write_filtered_project_fixture(root: &Path) -> PathBuf {
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let spanish_dir = locales_root.join("es");
        let french_dir = locales_root.join("fr");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&spanish_dir).expect("spanish locale dir");
        fs::create_dir_all(&french_dir).expect("french locale dir");

        write_catalog(
            &english_dir.join("common.json"),
            &[("home.title", "Hi"), ("cart.count", "{ $count :number }")],
        );
        write_catalog(&english_dir.join("admin.json"), &[("panel.title", "Admin")]);
        write_catalog(
            &spanish_dir.join("common.json"),
            &[("home.title", "Hola"), ("cart.count", "{ $count :number }")],
        );
        write_catalog(
            &spanish_dir.join("admin.json"),
            &[("panel.title", "Administracion")],
        );
        write_catalog(
            &french_dir.join("common.json"),
            &[
                ("home.title", "Salut"),
                ("cart.count", "{ $count :number }"),
            ],
        );
        write_catalog(&french_dir.join("admin.json"), &[("panel.title", "Admin")]);
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");
        config_path
    }

    #[test]
    fn builds_native_module_artifacts() {
        let root = temp_dir("success");
        let config_path = write_project_fixture(&root);
        let out_dir = root.join("out");

        let output = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            &out_dir,
            "app_i18n",
        ))
        .expect("build");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "fr".to_string()]
        );
        assert_eq!(
            output.default_catalog_keys(),
            &[
                "common.home.subtitle".to_string(),
                "common.home.title".to_string()
            ]
        );
        assert!(output.packs_dir().join("en.mf2pack").exists());
        assert!(output.packs_dir().join("fr.mf2pack").exists());
        assert!(output.id_map_path().exists());
        assert!(output.id_map_hash_path().exists());
        assert!(output.manifest_path().exists());
        assert!(output.platform_bundle_path().exists());
        assert!(output.generated_module_path().exists());
        assert!(output.generated_catalog_path().exists());
        assert!(output.rerun_if_changed_paths().contains(&config_path));
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("id_salt.txt"))
        );
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("locales").join("en").join("common.json"))
        );
        assert!(
            output
                .rerun_if_changed_paths()
                .contains(&root.join("locales").join("fr").join("common.json"))
        );

        let id_map_json = fs::read_to_string(output.id_map_path()).expect("id map");
        assert!(id_map_json.contains("\"common.home.title\""));

        let platform_bundle = PlatformBundle::load(output.platform_bundle_path()).expect("bundle");
        assert_eq!(platform_bundle.runtime_manifest().default_locale, "en");
        assert_eq!(
            platform_bundle
                .runtime_manifest()
                .mf2_packs
                .get("en")
                .expect("en pack")
                .url,
            "packs/en.mf2pack"
        );
        assert!(platform_bundle.pack("fr").is_some());

        let generated_module =
            fs::read_to_string(output.generated_module_path()).expect("generated module");
        assert!(generated_module.contains("mf2_i18n::define_i18n_module!"));
        assert!(generated_module.contains("/app_i18n/packs/en.mf2pack"));
        assert!(generated_module.contains("pub enum MessageKey"));
        assert!(generated_module.contains("CommonHomeTitle"));
        assert!(generated_module.contains("pub const DEFAULT_LOCALE: &str = \"en\""));
        assert!(generated_module.contains("pub const SUPPORTED_LOCALES: &[&str]"));
        assert!(generated_module.contains("pub fn try_t(key: MessageKey)"));
        assert!(generated_module.contains("pub fn t_or_key(key: MessageKey)"));
        assert!(generated_module.contains("impl mf2_i18n::I18nMessageKey for MessageKey"));
        assert!(generated_module.contains("pub fn try_t_with_args("));
        assert!(generated_module.contains("pub struct CommonHomeTitleArgs"));
        assert!(generated_module.contains("pub fn new(name: impl Into<String>)"));
        assert!(generated_module.contains("pub fn t_common_home_title_or_key("));
        assert!(generated_module.contains("pub fn t_common_home_subtitle()"));

        let generated_catalog =
            fs::read_to_string(output.generated_catalog_path()).expect("generated catalog");
        assert!(generated_catalog.contains("DEFAULT_LOCALE_ID"));
        assert!(generated_catalog.contains("\"fr\""));
        assert!(generated_catalog.contains("\"common.home.title\""));
        assert!(generated_catalog.contains("(\"common.home.title\", \"CommonHomeTitle\")"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn generated_key_collisions_fail_native_module_build() {
        let root = temp_dir("generated_key_collision");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        fs::create_dir_all(&english_dir).expect("english locale dir");

        write_catalog(
            &english_dir.join("common.json"),
            &[("home-title", "Hi"), ("home_title", "Hello")],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let err = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect_err("collision should fail");
        assert!(matches!(
            err,
            NativeModuleBuildError::GeneratedKeys(GeneratedKeyError::VariantCollision {
                variant,
                first_key,
                duplicate_key,
            }) if variant == "CommonHomeTitle"
                && first_key == "common.home-title"
                && duplicate_key == "common.home_title"
        ));
        assert!(
            !root
                .join("out")
                .join("app_i18n")
                .join(GENERATED_CATALOG_FILE)
                .exists()
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn generated_key_invalid_ids_fail_native_module_build() {
        let root = temp_dir("generated_key_invalid");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        fs::create_dir_all(&english_dir).expect("english locale dir");

        write_catalog(&english_dir.join("common.json"), &[("home/title", "Hi")]);
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let err = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect_err("invalid key should fail");
        assert!(matches!(
            err,
            NativeModuleBuildError::GeneratedKeys(GeneratedKeyError::InvalidMessageKey {
                key,
                ..
            }) if key == "common.home/title"
        ));
        assert!(
            !root
                .join("out")
                .join("app_i18n")
                .join(GENERATED_CATALOG_FILE)
                .exists()
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn generates_typed_args_for_formatter_types() {
        let root = temp_dir("typed_args");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        fs::create_dir_all(&english_dir).expect("english locale dir");

        write_catalog(
            &english_dir.join("common.json"),
            &[
                ("name", "Hi { $name }"),
                ("count", "{ $count :number }"),
                ("when", "{ $when :datetime }"),
                ("distance", "{ $distance :unit }"),
                ("total", "{ $total :currency }"),
                ("mixed", "{ $value } { $value :number }"),
            ],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let output = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect("build");
        let generated_module =
            fs::read_to_string(output.generated_module_path()).expect("generated module");

        assert!(generated_module.contains("pub struct CommonNameArgs"));
        assert!(generated_module.contains("name: impl Into<String>"));
        assert!(generated_module.contains("count: f64"));
        assert!(generated_module.contains("when: mf2_i18n::DateTimeValue"));
        assert!(generated_module.contains("distance_value: f64"));
        assert!(generated_module.contains("distance_unit_id: u32"));
        assert!(generated_module.contains("total_value: f64"));
        assert!(generated_module.contains("total_code: [u8; 3]"));
        assert!(generated_module.contains("value: mf2_i18n::Value"));
        assert!(generated_module.contains("pub fn t_common_total("));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn renders_typed_bool_args_from_metadata() {
        let key = GeneratedMessageKey {
            id: "common.enabled".to_string(),
            variant: "CommonEnabled".to_string(),
        };
        let source = render_generated_args_struct(
            &key,
            &[MessageArg {
                name: "enabled".to_string(),
                arg_type: ArgType::Bool,
                required: true,
            }],
        )
        .expect("render");

        assert!(source.contains("enabled: bool"));
        assert!(source.contains("mf2_i18n::Value::Bool(enabled)"));
    }

    #[test]
    fn runtime_artifacts_support_locale_and_namespace_filters() {
        let root = temp_dir("runtime_filter");
        let config_path = write_filtered_project_fixture(&root);
        let out_dir = root.join("runtime");

        let output = build_project_runtime_artifacts(
            &ProjectRuntimeBuildOptions::new(
                &config_path,
                &out_dir,
                "runtime-filter",
                "2026-02-01T00:00:00Z",
            )
            .with_locales(["en", "es"])
            .with_namespaces(["common"]),
        )
        .expect("build");

        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(
            output.default_catalog_keys(),
            &[
                "common.cart.count".to_string(),
                "common.home.title".to_string()
            ]
        );
        assert!(output.packs_dir().join("en.mf2pack").exists());
        assert!(output.packs_dir().join("es.mf2pack").exists());
        assert!(!output.packs_dir().join("fr.mf2pack").exists());

        let id_map_json = fs::read_to_string(output.id_map_path()).expect("id map");
        assert!(id_map_json.contains("\"common.home.title\""));
        assert!(!id_map_json.contains("\"admin.panel.title\""));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn supports_custom_module_macro_path() {
        let root = temp_dir("macro");
        let config_path = write_project_fixture(&root);
        let out_dir = root.join("out");

        let output = build_native_module(
            &NativeModuleBuildOptions::new(&config_path, &out_dir, "app_i18n")
                .with_module_macro_path("mf2_i18n_native::define_i18n_module!"),
        )
        .expect("build");

        let generated_module =
            fs::read_to_string(output.generated_module_path()).expect("generated module");
        assert!(generated_module.contains("mf2_i18n_native::define_i18n_module!"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn records_micro_locale_parent_packs() {
        let root = temp_dir("micro_locale");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let test_dir = locales_root.join("en-x-test");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&test_dir).expect("test locale dir");
        write_catalog(&english_dir.join("common.json"), &[("home.title", "Hi")]);
        write_catalog(&test_dir.join("common.json"), &[("home.title", "Test")]);
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");
        fs::write(
            root.join("micro-locales.toml"),
            "[[locale]]\ntag = \"en-x-test\"\nparent = \"en\"\n",
        )
        .expect("micro locales");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nmicro_locales_registry = \"micro-locales.toml\"\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let output = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect("build");
        let platform_bundle = PlatformBundle::load(output.platform_bundle_path()).expect("bundle");
        let pack = platform_bundle.pack("en-x-test").expect("pack");
        assert_eq!(pack.entry.kind, "overlay");
        assert_eq!(pack.entry.parent.as_deref(), Some("en"));
        assert_eq!(
            platform_bundle
                .runtime_manifest()
                .micro_locales
                .as_ref()
                .and_then(|parents| parents.get("en-x-test"))
                .map(String::as_str),
            Some("en")
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_mismatched_catalog_keys() {
        let root = temp_dir("mismatch");
        let locales_root = root.join("locales");
        let english_dir = locales_root.join("en");
        let french_dir = locales_root.join("fr");
        fs::create_dir_all(&english_dir).expect("english locale dir");
        fs::create_dir_all(&french_dir).expect("french locale dir");

        write_catalog(&english_dir.join("common.json"), &[("home.title", "Hi")]);
        write_catalog(
            &french_dir.join("common.json"),
            &[("home.subtitle", "Salut")],
        );
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");

        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");

        let err = build_native_module(&NativeModuleBuildOptions::new(
            &config_path,
            root.join("out"),
            "app_i18n",
        ))
        .expect_err("mismatch should fail");
        assert!(matches!(
            err,
            NativeModuleBuildError::Catalogs(ProjectCatalogError::CatalogKeyMismatch {
                locale,
                reference_locale,
                ..
            }) if locale == "fr" && reference_locale == "en"
        ));

        fs::remove_dir_all(&root).ok();
    }
}

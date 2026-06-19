use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
use crate::project_catalogs::{
    ProjectCatalog, ProjectCatalogError, ProjectCatalogLoadOptions,
    load_project_catalogs_with_options,
};
use crate::web::{
    WebArtifactError, WebArtifactFile, WebArtifactManifestInput, WebArtifactWriteOptions,
    WebArtifactWriteOutput, WebDtoTypeArtifactError, append_web_dto_artifacts, write_web_artifacts,
};
use crate::web_metadata::{
    WebMessageMetadata, WebMessageMetadataError, collect_web_message_metadata,
};
use crate::{WebStalePolicy, WebTargetConfigError, WebTargetMode};

const DOCS_NAMESPACE: &str = "docs";
const DOCUSAURUS_NAMESPACE: &str = "docusaurus";
const DEFAULT_SIDEBAR_ID: &str = "docsSidebar";
const DOCUSAURUS_DOCS_PLUGIN: &str = "docusaurus-plugin-content-docs/current";
const MANAGED_STATE_FILE: &str = ".mf2-managed.json";

#[derive(Debug, Clone)]
pub struct DocusaurusWebBuildOptions {
    config_path: PathBuf,
    target_name: String,
    out_dir_override: Option<PathBuf>,
    check: bool,
    dto_types: bool,
}

impl DocusaurusWebBuildOptions {
    pub fn new(config_path: impl Into<PathBuf>, target_name: impl Into<String>) -> Self {
        Self {
            config_path: config_path.into(),
            target_name: target_name.into(),
            out_dir_override: None,
            check: false,
            dto_types: false,
        }
    }

    pub fn with_out_dir(mut self, out_dir: impl Into<PathBuf>) -> Self {
        self.out_dir_override = Some(out_dir.into());
        self
    }

    pub fn with_check(mut self, check: bool) -> Self {
        self.check = check;
        self
    }

    pub fn with_dto_types(mut self, dto_types: bool) -> Self {
        self.dto_types = dto_types;
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn target_name(&self) -> &str {
        &self.target_name
    }

    pub fn out_dir_override(&self) -> Option<&Path> {
        self.out_dir_override.as_deref()
    }

    pub fn check(&self) -> bool {
        self.check
    }

    pub fn dto_types(&self) -> bool {
        self.dto_types
    }
}

#[derive(Debug, Clone)]
pub struct DocusaurusWebBuildOutput {
    artifact_output: WebArtifactWriteOutput,
    i18n_root: PathBuf,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    message_metadata: Vec<WebMessageMetadata>,
}

impl DocusaurusWebBuildOutput {
    pub fn artifact_output(&self) -> &WebArtifactWriteOutput {
        &self.artifact_output
    }

    pub fn i18n_root(&self) -> &Path {
        &self.i18n_root
    }

    pub fn default_locale(&self) -> &str {
        &self.default_locale
    }

    pub fn supported_locales(&self) -> &[String] {
        &self.supported_locales
    }

    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    pub fn message_metadata(&self) -> &[WebMessageMetadata] {
        &self.message_metadata
    }
}

#[derive(Debug, Error)]
pub enum DocusaurusWebBuildError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] WebTargetConfigError),
    #[error(transparent)]
    Catalogs(#[from] ProjectCatalogError),
    #[error(transparent)]
    Metadata(#[from] WebMessageMetadataError),
    #[error(transparent)]
    Artifacts(#[from] WebArtifactError),
    #[error(transparent)]
    DtoTypes(#[from] WebDtoTypeArtifactError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("missing web target {0}")]
    MissingTarget(String),
    #[error("web target {target} must use docusaurus mode")]
    UnsupportedTargetMode { target: String },
    #[error("docusaurus web target {target} is missing docs_source_dir")]
    MissingDocsSourceDir { target: String },
    #[error("docusaurus web target {target} is missing docusaurus_i18n_dir")]
    MissingI18nDir { target: String },
    #[error("docusaurus output drift for {0:?}")]
    CheckFailed(PathBuf),
    #[error("docusaurus default locale {locale} must not define translated docs sources")]
    DefaultLocaleDocsCatalog {
        locale: String,
        source_paths: Vec<PathBuf>,
    },
    #[error("docusaurus locale {locale} must define docs translations as docs/**/*.mf2 files")]
    FlatDocsCatalog {
        locale: String,
        source_paths: Vec<PathBuf>,
    },
    #[error("docusaurus locale {locale} docs catalog does not match docs_source_dir")]
    TranslatedDocsMismatch {
        locale: String,
        missing: Vec<String>,
        extra: Vec<String>,
    },
    #[error(
        "duplicate docusaurus translated doc {doc_path} for locale {locale}: {first_path:?} and {duplicate_path:?}"
    )]
    DuplicateTranslatedDoc {
        locale: String,
        doc_path: String,
        first_path: PathBuf,
        duplicate_path: PathBuf,
    },
    #[error("docusaurus translated doc source {path:?} must define a content entry")]
    MissingDocContent { path: PathBuf },
    #[error("docusaurus translated doc source {path:?} line {line} must contain '='")]
    InvalidDocContentHeader { path: PathBuf, line: u32 },
    #[error(
        "docusaurus translated doc source {path:?} line {line} must use content key, found {key}"
    )]
    InvalidDocContentKey {
        path: PathBuf,
        line: u32,
        key: String,
    },
    #[error("docusaurus docs source path is not UTF-8: {0:?}")]
    NonUtf8DocPath(PathBuf),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ManagedState {
    #[serde(default)]
    code: Vec<String>,
    #[serde(default)]
    docs: Vec<String>,
    #[serde(default)]
    targets: BTreeMap<String, Vec<String>>,
}

pub fn build_docusaurus_web_target(
    options: &DocusaurusWebBuildOptions,
) -> Result<DocusaurusWebBuildOutput, DocusaurusWebBuildError> {
    let project = ProjectLayout::load_or_default(options.config_path())?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(options.target_name())
        .ok_or_else(|| DocusaurusWebBuildError::MissingTarget(options.target_name().to_owned()))?
        .clone();
    if target.mode != WebTargetMode::Docusaurus {
        return Err(DocusaurusWebBuildError::UnsupportedTargetMode {
            target: target.name.clone(),
        });
    }

    let manages_docs = target.namespaces.is_empty()
        || target
            .namespaces
            .iter()
            .any(|namespace| namespace == DOCS_NAMESPACE);
    let loaded_catalogs = load_project_catalogs_with_options(
        &project,
        &ProjectCatalogLoadOptions::default().ignore_key_match_namespace(DOCS_NAMESPACE),
    )?;
    let catalogs = filtered_catalogs(
        loaded_catalogs.catalogs(),
        &target.locales,
        &target.namespaces,
    );
    let default_locale = project.config().default_locale.clone();
    let default_catalog = catalogs
        .get(&default_locale)
        .ok_or_else(|| ProjectCatalogError::MissingDefaultLocale(default_locale.clone()))?;
    let namespace_filter = (!target.namespaces.is_empty()).then_some(target.namespaces.as_slice());
    let message_metadata =
        collect_web_message_metadata(&default_locale, default_catalog, namespace_filter)?;
    let supported_locales = catalogs.keys().cloned().collect::<Vec<_>>();
    let i18n_root = resolve_config_relative_path(
        options.config_path(),
        target.docusaurus_i18n_dir.as_deref().ok_or_else(|| {
            DocusaurusWebBuildError::MissingI18nDir {
                target: target.name.clone(),
            }
        })?,
    );
    let docs_source_dir = resolve_config_relative_path(
        options.config_path(),
        target.docs_source_dir.as_deref().ok_or_else(|| {
            DocusaurusWebBuildError::MissingDocsSourceDir {
                target: target.name.clone(),
            }
        })?,
    );
    let sidebar_id = target
        .docusaurus_sidebar_id
        .as_deref()
        .unwrap_or(DEFAULT_SIDEBAR_ID);
    let source_docs = if manages_docs {
        discover_source_docs(&docs_source_dir)?
    } else {
        BTreeSet::new()
    };
    let translated_docs_by_locale = if manages_docs {
        load_translated_doc_sources(&project, &supported_locales, &default_locale)?
    } else {
        BTreeMap::new()
    };

    let previous = read_managed_state(&i18n_root)?;
    let generated = GeneratedDocusaurus::from_catalogs(
        &default_locale,
        &catalogs,
        &source_docs,
        &translated_docs_by_locale,
        manages_docs,
        sidebar_id,
    )?;
    let namespaces = generated_namespaces(&message_metadata, manages_docs);
    write_docusaurus_outputs(
        &i18n_root,
        &default_locale,
        &generated,
        &previous,
        options.check(),
    )?;

    let out_dir = options
        .out_dir_override()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_config_relative_path(options.config_path(), &target.out_dir));
    let mut artifact_files = vec![WebArtifactFile::new(
        "messages.ts",
        render_messages_module(
            &default_locale,
            &supported_locales,
            &generated.code_by_locale,
        )
        .into_bytes(),
    )?];
    if options.dto_types() {
        append_web_dto_artifacts(&mut artifact_files)?;
    }

    let artifact_output = write_web_artifacts(
        &WebArtifactWriteOptions::new(out_dir)
            .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
            .with_check(options.check()),
        &WebArtifactManifestInput {
            target: target.name.clone(),
            mode: target.mode,
            framework: target.framework,
            default_locale: default_locale.clone(),
            supported_locales: supported_locales.clone(),
            namespaces: namespaces.clone(),
        },
        artifact_files,
    )?;

    Ok(DocusaurusWebBuildOutput {
        artifact_output,
        i18n_root,
        default_locale,
        supported_locales,
        namespaces,
        message_metadata,
    })
}

struct GeneratedDocusaurus {
    code_by_locale: BTreeMap<String, BTreeMap<String, String>>,
    docs_by_locale: BTreeMap<String, BTreeMap<String, String>>,
    targets_by_locale: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>,
    managed_code: Vec<String>,
    managed_docs: Vec<String>,
    managed_targets: BTreeMap<String, Vec<String>>,
}

impl GeneratedDocusaurus {
    fn from_catalogs(
        default_locale: &str,
        catalogs: &BTreeMap<String, ProjectCatalog>,
        source_docs: &BTreeSet<String>,
        translated_docs_by_locale: &BTreeMap<String, BTreeMap<String, String>>,
        manages_docs: bool,
        sidebar_id: &str,
    ) -> Result<Self, DocusaurusWebBuildError> {
        let mut code_by_locale = BTreeMap::new();
        let mut docs_by_locale = BTreeMap::new();
        let mut targets_by_locale = BTreeMap::new();
        let mut default_docs_source_paths = BTreeSet::new();
        let mut flat_docs_catalogs = BTreeMap::<String, BTreeSet<PathBuf>>::new();

        for (locale, catalog) in catalogs {
            let mut code = BTreeMap::new();
            let docs = translated_docs_by_locale
                .get(locale)
                .cloned()
                .unwrap_or_default();
            let mut targets = BTreeMap::<String, BTreeMap<String, String>>::new();

            for message in catalog.values() {
                match message.namespace.as_str() {
                    DOCS_NAMESPACE => {
                        if locale == default_locale {
                            default_docs_source_paths.insert(message.source_path.clone());
                        } else {
                            flat_docs_catalogs
                                .entry(locale.clone())
                                .or_default()
                                .insert(message.source_path.clone());
                        }
                    }
                    DOCUSAURUS_NAMESPACE => {
                        if let Some((target_path, id)) =
                            docusaurus_target_id(&message.message_path, sidebar_id)
                        {
                            targets
                                .entry(target_path)
                                .or_default()
                                .insert(id, message.value.clone());
                        }
                    }
                    _ => {
                        code.insert(message.qualified_key.clone(), message.value.clone());
                    }
                }
            }

            code_by_locale.insert(locale.clone(), code);
            docs_by_locale.insert(locale.clone(), docs);
            targets_by_locale.insert(locale.clone(), targets);
        }

        if let Some((locale, source_paths)) = flat_docs_catalogs.into_iter().next() {
            return Err(DocusaurusWebBuildError::FlatDocsCatalog {
                locale,
                source_paths: source_paths.into_iter().collect(),
            });
        }

        if !default_docs_source_paths.is_empty() {
            return Err(DocusaurusWebBuildError::DefaultLocaleDocsCatalog {
                locale: default_locale.to_string(),
                source_paths: default_docs_source_paths.into_iter().collect(),
            });
        }

        let managed_code = code_by_locale
            .get(default_locale)
            .map(|messages| messages.keys().cloned().collect())
            .unwrap_or_default();
        let managed_docs = source_docs.iter().cloned().collect::<Vec<_>>();
        let mut managed_targets = BTreeMap::new();
        if let Some(targets) = targets_by_locale.get(default_locale) {
            for (target_path, ids) in targets {
                managed_targets.insert(target_path.clone(), ids.keys().cloned().collect());
            }
        }

        if manages_docs {
            for (locale, docs) in &docs_by_locale {
                if locale == default_locale {
                    continue;
                }
                let doc_paths = docs.keys().cloned().collect::<BTreeSet<_>>();
                let missing = source_docs
                    .difference(&doc_paths)
                    .cloned()
                    .collect::<Vec<_>>();
                let extra = doc_paths
                    .difference(source_docs)
                    .cloned()
                    .collect::<Vec<_>>();
                if !missing.is_empty() || !extra.is_empty() {
                    return Err(DocusaurusWebBuildError::TranslatedDocsMismatch {
                        locale: locale.clone(),
                        missing,
                        extra,
                    });
                }
            }
        }

        Ok(Self {
            code_by_locale,
            docs_by_locale,
            targets_by_locale,
            managed_code,
            managed_docs,
            managed_targets,
        })
    }
}

fn write_docusaurus_outputs(
    i18n_root: &Path,
    default_locale: &str,
    generated: &GeneratedDocusaurus,
    previous: &ManagedState,
    check: bool,
) -> Result<(), DocusaurusWebBuildError> {
    for (locale, code) in &generated.code_by_locale {
        if locale == default_locale {
            continue;
        }
        let code_path = i18n_root.join(locale).join("code.json");
        let next = merge_code_json(
            read_json_object(&code_path)?,
            code,
            &generated.managed_code,
            &previous.code,
            "code.json",
        );
        write_json_or_check(&code_path, &Value::Object(next), check)?;
    }

    for (locale, docs) in &generated.docs_by_locale {
        if locale == default_locale {
            continue;
        }
        let docs_root = i18n_root.join(locale).join(DOCUSAURUS_DOCS_PLUGIN);
        for doc_path in &previous.docs {
            if !generated.managed_docs.contains(doc_path) {
                let stale_path = docs_root.join(doc_path);
                if !check {
                    fs::remove_file(stale_path).ok();
                }
            }
        }
        for (doc_path, contents) in docs {
            write_bytes_or_check(
                &docs_root.join(doc_path),
                format!("{contents}\n").as_bytes(),
                check,
            )?;
        }
    }

    for (locale, targets) in &generated.targets_by_locale {
        if locale == default_locale {
            continue;
        }
        for (target_path, entries) in targets {
            let output_path = i18n_root.join(locale).join(target_path);
            let previous_ids = previous
                .targets
                .get(target_path)
                .cloned()
                .unwrap_or_default();
            let managed_ids = generated
                .managed_targets
                .get(target_path)
                .cloned()
                .unwrap_or_default();
            let next = merge_code_json(
                read_json_object(&output_path)?,
                entries,
                &managed_ids,
                &previous_ids,
                target_path,
            );
            write_json_or_check(&output_path, &Value::Object(next), check)?;
        }
    }

    write_json_or_check(
        &i18n_root.join(MANAGED_STATE_FILE),
        &serde_json::to_value(ManagedState {
            code: generated.managed_code.clone(),
            docs: generated.managed_docs.clone(),
            targets: generated.managed_targets.clone(),
        })?,
        check,
    )
}

fn merge_code_json(
    existing: Map<String, Value>,
    messages: &BTreeMap<String, String>,
    managed_ids: &[String],
    previous_ids: &[String],
    target_path: &str,
) -> Map<String, Value> {
    let mut next = existing;
    for id in previous_ids {
        if !managed_ids.contains(id) {
            next.remove(id);
        }
    }
    for id in managed_ids {
        if let Some(message) = messages.get(id) {
            next.insert(
                id.clone(),
                serde_json::json!({
                    "message": message,
                    "description": format!("MF2-managed Docusaurus string from {id} for {target_path}."),
                }),
            );
        }
    }
    sort_json_map(next)
}

fn read_managed_state(i18n_root: &Path) -> Result<ManagedState, DocusaurusWebBuildError> {
    let path = i18n_root.join(MANAGED_STATE_FILE);
    if !path.exists() {
        return Ok(ManagedState::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn read_json_object(path: &Path) -> Result<Map<String, Value>, DocusaurusWebBuildError> {
    if !path.exists() {
        return Ok(Map::new());
    }
    match serde_json::from_slice::<Value>(&fs::read(path)?)? {
        Value::Object(object) => Ok(object),
        _ => Ok(Map::new()),
    }
}

fn write_json_or_check(
    path: &Path,
    value: &Value,
    check: bool,
) -> Result<(), DocusaurusWebBuildError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_bytes_or_check(path, &bytes, check)
}

fn write_bytes_or_check(
    path: &Path,
    bytes: &[u8],
    check: bool,
) -> Result<(), DocusaurusWebBuildError> {
    if check {
        let existing = fs::read(path).unwrap_or_default();
        if existing != bytes {
            return Err(DocusaurusWebBuildError::CheckFailed(path.to_path_buf()));
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(())
}

fn render_messages_module(
    default_locale: &str,
    supported_locales: &[String],
    code_by_locale: &BTreeMap<String, BTreeMap<String, String>>,
) -> String {
    let default_messages = code_by_locale
        .get(default_locale)
        .cloned()
        .unwrap_or_default();
    format!(
        "export const DEFAULT_LOCALE = {} as const;\nexport const SUPPORTED_LOCALES = {} as const;\nexport const MESSAGES = {} as const;\nexport type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];\nexport type Mf2MessageId = keyof (typeof MESSAGES)[typeof DEFAULT_LOCALE];\nexport const DEFAULT_MESSAGES = {} as const satisfies Record<Mf2MessageId, string>;\n",
        json_string(default_locale),
        serde_json::to_string(supported_locales).expect("locales"),
        serde_json::to_string_pretty(code_by_locale).expect("messages"),
        serde_json::to_string_pretty(&default_messages).expect("default messages"),
    )
}

fn filtered_catalogs(
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

fn namespaces_from_metadata(message_metadata: &[WebMessageMetadata]) -> Vec<String> {
    message_metadata
        .iter()
        .map(|message| message.namespace.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn generated_namespaces(
    message_metadata: &[WebMessageMetadata],
    manages_docs: bool,
) -> Vec<String> {
    let mut namespaces = namespaces_from_metadata(message_metadata)
        .into_iter()
        .collect::<BTreeSet<_>>();
    if manages_docs {
        namespaces.insert(DOCS_NAMESPACE.to_string());
    }
    namespaces.into_iter().collect()
}

fn discover_source_docs(
    docs_source_dir: &Path,
) -> Result<BTreeSet<String>, DocusaurusWebBuildError> {
    let mut docs = BTreeSet::new();
    collect_source_docs(docs_source_dir, docs_source_dir, &mut docs)?;
    Ok(docs)
}

fn collect_source_docs(
    root: &Path,
    current: &Path,
    docs: &mut BTreeSet<String>,
) -> Result<(), DocusaurusWebBuildError> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_source_docs(root, &path, docs)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("mdx") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| DocusaurusWebBuildError::NonUtf8DocPath(path.clone()))?;
        let relative = relative
            .to_str()
            .ok_or_else(|| DocusaurusWebBuildError::NonUtf8DocPath(path.clone()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        docs.insert(relative);
    }
    Ok(())
}

fn load_translated_doc_sources(
    project: &ProjectLayout,
    supported_locales: &[String],
    default_locale: &str,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, DocusaurusWebBuildError> {
    let supported = supported_locales.iter().cloned().collect::<BTreeSet<_>>();
    let mut docs_by_locale = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut doc_sources_by_locale = BTreeMap::<String, BTreeMap<String, PathBuf>>::new();

    for source_root in project.source_roots() {
        let mut locale_entries = fs::read_dir(&source_root)?.collect::<Result<Vec<_>, _>>()?;
        locale_entries.sort_by_key(|entry| entry.path());
        for entry in locale_entries {
            let locale_dir = entry.path();
            if !locale_dir.is_dir() {
                continue;
            }
            let locale = entry.file_name().to_string_lossy().into_owned();
            if !supported.contains(&locale) {
                continue;
            }
            let docs_root = locale_dir.join(DOCS_NAMESPACE);
            if !docs_root.exists() {
                continue;
            }

            let source_paths = discover_translated_doc_sources(&docs_root)?;
            if locale == default_locale {
                if !source_paths.is_empty() {
                    return Err(DocusaurusWebBuildError::DefaultLocaleDocsCatalog {
                        locale,
                        source_paths,
                    });
                }
                continue;
            }

            for source_path in source_paths {
                let doc_path = translated_doc_output_path(&docs_root, &source_path)?;
                let value = read_translated_doc_content(&source_path)?;
                let locale_docs = docs_by_locale.entry(locale.clone()).or_default();
                let locale_sources = doc_sources_by_locale.entry(locale.clone()).or_default();
                if let Some(first_path) = locale_sources.get(&doc_path) {
                    return Err(DocusaurusWebBuildError::DuplicateTranslatedDoc {
                        locale,
                        doc_path,
                        first_path: first_path.clone(),
                        duplicate_path: source_path,
                    });
                }
                locale_sources.insert(doc_path.clone(), source_path);
                locale_docs.insert(doc_path, value);
            }
        }
    }

    Ok(docs_by_locale)
}

fn discover_translated_doc_sources(
    docs_root: &Path,
) -> Result<Vec<PathBuf>, DocusaurusWebBuildError> {
    let mut sources = Vec::new();
    collect_translated_doc_sources(docs_root, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn collect_translated_doc_sources(
    current: &Path,
    sources: &mut Vec<PathBuf>,
) -> Result<(), DocusaurusWebBuildError> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_translated_doc_sources(&path, sources)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("mf2") {
            sources.push(path);
        }
    }
    Ok(())
}

fn translated_doc_output_path(
    docs_root: &Path,
    source_path: &Path,
) -> Result<String, DocusaurusWebBuildError> {
    let relative = source_path
        .strip_prefix(docs_root)
        .map_err(|_| DocusaurusWebBuildError::NonUtf8DocPath(source_path.to_path_buf()))?;
    let relative = relative
        .to_str()
        .ok_or_else(|| DocusaurusWebBuildError::NonUtf8DocPath(source_path.to_path_buf()))?
        .replace(std::path::MAIN_SEPARATOR, "/");
    let Some(stem) = relative.strip_suffix(".mf2") else {
        return Err(DocusaurusWebBuildError::NonUtf8DocPath(
            source_path.to_path_buf(),
        ));
    };
    Ok(format!("{stem}.mdx"))
}

fn read_translated_doc_content(path: &Path) -> Result<String, DocusaurusWebBuildError> {
    let raw = fs::read_to_string(path)?;
    let mut offset = 0usize;

    for (index, segment) in raw.split_inclusive('\n').enumerate() {
        let line_number = (index + 1) as u32;
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim();
        let line_start = offset;
        offset += segment.len();

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(DocusaurusWebBuildError::InvalidDocContentHeader {
                path: path.to_path_buf(),
                line: line_number,
            });
        };
        let key = key.trim();
        if key != "content" {
            return Err(DocusaurusWebBuildError::InvalidDocContentKey {
                path: path.to_path_buf(),
                line: line_number,
                key: key.to_string(),
            });
        }

        let inline_leading = value.len() - value.trim_start().len();
        let equals_index = line.find('=').expect("split_once found an equals sign");
        let inline_start = line_start + equals_index + 1 + inline_leading;
        let line_end = line_start + line.len();
        let mut content = raw[inline_start..line_end].to_string();
        let rest = &raw[offset..];
        if !rest.is_empty() {
            if !content.is_empty() {
                content.push('\n');
            }
            content.push_str(rest);
        }
        return Ok(content.trim_end().to_string());
    }

    Err(DocusaurusWebBuildError::MissingDocContent {
        path: path.to_path_buf(),
    })
}

fn docusaurus_target_id(message_path: &str, sidebar_id: &str) -> Option<(String, String)> {
    if let Some(label) = message_path.strip_prefix("navbar.") {
        let id = if label == "logo_alt" {
            "logo.alt".to_string()
        } else {
            format!("item.label.{}", label_id(label))
        };
        return Some(("docusaurus-theme-classic/navbar.json".to_string(), id));
    }
    if let Some(label) = message_path.strip_prefix("docs.sidebar.") {
        return Some((
            "docusaurus-plugin-content-docs/current.json".to_string(),
            format!("sidebar.{sidebar_id}.category.{}", label_id(label)),
        ));
    }
    None
}

fn label_id(value: &str) -> String {
    match value {
        "github" => "GitHub".to_string(),
        "ios" => "iOS".to_string(),
        "sdk" => "SDK".to_string(),
        _ => value
            .split(['_', '-'])
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn sort_json_map(map: Map<String, Value>) -> Map<String, Value> {
    map.into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect()
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}

#[cfg(test)]
mod tests {
    use super::{DocusaurusWebBuildError, DocusaurusWebBuildOptions, build_docusaurus_web_target};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_docusaurus_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_fixture(root: &Path) -> PathBuf {
        for locale in ["en", "es"] {
            fs::create_dir_all(root.join("locales").join(locale)).expect("locale");
        }
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");
        fs::create_dir_all(root.join("docs").join("radroots")).expect("docs");
        fs::write(root.join("docs/radroots/introduction.mdx"), "# Intro\n").expect("doc");
        fs::create_dir_all(root.join("locales/es/docs/radroots")).expect("docs es dir");
        fs::write(
            root.join("locales/en/home.mf2"),
            "head.title = Developer Docs\n",
        )
        .expect("home en");
        fs::write(
            root.join("locales/es/home.mf2"),
            "head.title = Documentacion\n",
        )
        .expect("home es");
        fs::write(
            root.join("locales/es/docs/radroots/introduction.mf2"),
            "content = ---\ntitle: Introduccion\n---\n# Introduccion\n\nContenido traducido.\n",
        )
        .expect("docs es");
        fs::write(
            root.join("locales/en/docusaurus.mf2"),
            "navbar.overview = Overview\n\ndocs.sidebar.overview = Overview\n",
        )
        .expect("docusaurus en");
        fs::write(
            root.join("locales/es/docusaurus.mf2"),
            "navbar.overview = Introduccion\n\ndocs.sidebar.overview = Introduccion\n",
        )
        .expect("docusaurus es");
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"docs\"\nmode = \"docusaurus\"\nframework = \"docusaurus\"\nout_dir = \"src/generated/mf2\"\ndocs_source_dir = \"docs\"\ndocusaurus_i18n_dir = \"i18n\"\ndocusaurus_sidebar_id = \"radrootsSidebar\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"home\", \"docs\", \"docusaurus\"]\nstale_policy = \"fail\"\n",
        )
        .expect("config");
        config_path
    }

    #[test]
    fn builds_docusaurus_native_outputs() {
        let root = temp_dir("target");
        let config_path = write_fixture(&root);

        let output =
            build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&config_path, "docs"))
                .expect("build");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(
            output.namespaces(),
            &[
                "docs".to_string(),
                "docusaurus".to_string(),
                "home".to_string()
            ]
        );
        assert_eq!(output.message_metadata().len(), 3);
        assert!(output.artifact_output().manifest_path().exists());
        assert!(output.i18n_root().join(".mf2-managed.json").exists());

        let code_json = fs::read_to_string(root.join("i18n/es/code.json")).expect("code");
        assert!(code_json.contains("\"home.head.title\""));
        assert!(code_json.contains("Documentacion"));

        let doc = fs::read_to_string(
            root.join("i18n/es/docusaurus-plugin-content-docs/current/radroots/introduction.mdx"),
        )
        .expect("doc");
        assert!(doc.contains("# Introduccion"));
        assert!(doc.contains("Contenido traducido."));

        let navbar = fs::read_to_string(root.join("i18n/es/docusaurus-theme-classic/navbar.json"))
            .expect("navbar");
        assert!(navbar.contains("\"item.label.Overview\""));

        let current_json =
            fs::read_to_string(root.join("i18n/es/docusaurus-plugin-content-docs/current.json"))
                .expect("current json");
        assert!(current_json.contains("sidebar.radrootsSidebar.category.Overview"));

        let messages_ts =
            fs::read_to_string(root.join("src/generated/mf2/messages.ts")).expect("messages");
        assert!(messages_ts.contains("SUPPORTED_LOCALES"));
        assert!(messages_ts.contains("home.head.title"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_default_locale_docs_catalog() {
        let root = temp_dir("default_docs_rejected");
        let config_path = write_fixture(&root);
        fs::create_dir_all(root.join("locales/en/docs/radroots")).expect("docs en dir");
        fs::write(
            root.join("locales/en/docs/radroots/introduction.mf2"),
            "content = ---\ntitle: Introduction\n---\n# Introduction\n",
        )
        .expect("docs en");

        let err =
            build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&config_path, "docs"))
                .expect_err("default docs should fail");

        assert!(matches!(
            err,
            DocusaurusWebBuildError::DefaultLocaleDocsCatalog { locale, .. }
                if locale == "en"
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_flat_docs_catalog() {
        let root = temp_dir("flat_docs_rejected");
        let config_path = write_fixture(&root);
        fs::write(
            root.join("locales/es/docs.mf2"),
            "radroots.introduction = ---\ntitle: Introduccion\n---\n# Introduccion\n",
        )
        .expect("docs es");

        let err =
            build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&config_path, "docs"))
                .expect_err("flat docs should fail");

        assert!(matches!(
            err,
            DocusaurusWebBuildError::FlatDocsCatalog { locale, .. }
                if locale == "es"
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_missing_translated_doc() {
        let root = temp_dir("missing_translated_doc");
        let config_path = write_fixture(&root);
        fs::write(
            root.join("docs/radroots/architecture.mdx"),
            "# Architecture\n",
        )
        .expect("source doc");

        let err =
            build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&config_path, "docs"))
                .expect_err("missing doc should fail");

        assert!(matches!(
            err,
            DocusaurusWebBuildError::TranslatedDocsMismatch { locale, missing, extra }
                if locale == "es"
                    && missing == vec!["radroots/architecture.mdx".to_string()]
                    && extra.is_empty()
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_extra_translated_doc() {
        let root = temp_dir("extra_translated_doc");
        let config_path = write_fixture(&root);
        fs::create_dir_all(root.join("locales/es/docs/radroots")).expect("docs es dir");
        fs::write(
            root.join("locales/es/docs/radroots/extra.mf2"),
            "content = ---\ntitle: Extra\n---\n# Extra\n",
        )
        .expect("extra docs es");

        let err =
            build_docusaurus_web_target(&DocusaurusWebBuildOptions::new(&config_path, "docs"))
                .expect_err("extra doc should fail");

        assert!(matches!(
            err,
            DocusaurusWebBuildError::TranslatedDocsMismatch { locale, missing, extra }
                if locale == "es"
                    && missing.is_empty()
                    && extra == vec!["radroots/extra.mdx".to_string()]
        ));

        fs::remove_dir_all(&root).ok();
    }
}

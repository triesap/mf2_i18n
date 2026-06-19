use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::{Map, Value};
use thiserror::Error;

use crate::parser::{Expr, Message, Segment, SelectKind, parse_message};
use crate::project::{ProjectError, ProjectLayout, resolve_config_relative_path};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogError, ProjectCatalogMessage};
use crate::web::{
    WebArtifactError, WebArtifactFile, WebArtifactManifestInput, WebArtifactWriteOptions,
    WebArtifactWriteOutput, WebDtoTypeArtifactError, append_web_dto_artifacts, write_web_artifacts,
};
use crate::web_metadata::{
    WebMessageMetadata, WebMessageMetadataError, collect_web_message_metadata,
    render_web_typescript_metadata_types,
};
use crate::{WebStalePolicy, WebTargetConfig, WebTargetConfigError, WebTargetMode};

const MESSAGES_DIR: &str = "messages";
const TYPESCRIPT_MANIFEST_FILE: &str = "i18n-manifest.ts";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebJsonMode {
    Plain,
}

impl FromStr for WebJsonMode {
    type Err = WebJsonModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "plain" => Ok(Self::Plain),
            _ => Err(WebJsonModeParseError::Unsupported(value.to_owned())),
        }
    }
}

impl fmt::Display for WebJsonMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Plain => formatter.write_str("plain"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WebJsonModeParseError {
    #[error("unsupported web JSON mode {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone)]
pub struct WebJsonExportOptions {
    config_path: PathBuf,
    out_dir: PathBuf,
    mode: WebJsonMode,
}

impl WebJsonExportOptions {
    pub fn new(config_path: impl Into<PathBuf>, out_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            out_dir: out_dir.into(),
            mode: WebJsonMode::Plain,
        }
    }

    pub fn with_mode(mut self, mode: WebJsonMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn mode(&self) -> WebJsonMode {
        self.mode
    }
}

#[derive(Debug, Clone)]
pub struct PlainJsonWebBuildOptions {
    config_path: PathBuf,
    target_name: String,
    out_dir_override: Option<PathBuf>,
    check: bool,
    dto_types: bool,
}

impl PlainJsonWebBuildOptions {
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
pub struct WebJsonMessageFile {
    locale: String,
    namespace: String,
    path: PathBuf,
}

impl WebJsonMessageFile {
    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub struct WebJsonExportOutput {
    out_dir: PathBuf,
    messages_dir: PathBuf,
    manifest_path: PathBuf,
    message_files: Vec<WebJsonMessageFile>,
    rerun_if_changed_paths: Vec<PathBuf>,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    mode: WebJsonMode,
    message_metadata: Vec<WebMessageMetadata>,
}

impl WebJsonExportOutput {
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn messages_dir(&self) -> &Path {
        &self.messages_dir
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub fn message_files(&self) -> &[WebJsonMessageFile] {
        &self.message_files
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

    pub fn namespaces(&self) -> &[String] {
        &self.namespaces
    }

    pub fn mode(&self) -> WebJsonMode {
        self.mode
    }

    pub fn message_metadata(&self) -> &[WebMessageMetadata] {
        &self.message_metadata
    }
}

#[derive(Debug, Clone)]
pub struct PlainJsonWebBuildOutput {
    artifact_output: WebArtifactWriteOutput,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    message_metadata: Vec<WebMessageMetadata>,
}

impl PlainJsonWebBuildOutput {
    pub fn artifact_output(&self) -> &WebArtifactWriteOutput {
        &self.artifact_output
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebJsonUnsupportedKind {
    Variable,
    Formatter,
    Select,
    Plural,
}

impl fmt::Display for WebJsonUnsupportedKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Variable => formatter.write_str("variable"),
            Self::Formatter => formatter.write_str("formatter"),
            Self::Select => formatter.write_str("select"),
            Self::Plural => formatter.write_str("plural"),
        }
    }
}

#[derive(Debug, Error)]
pub enum WebJsonExportError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Catalogs(#[from] ProjectCatalogError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Metadata(#[from] WebMessageMetadataError),
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
    #[error(
        "plain web JSON does not support {kind} messages for locale {locale} key {key} at {line}:{column}"
    )]
    UnsupportedMessage {
        locale: String,
        key: String,
        kind: WebJsonUnsupportedKind,
        line: u32,
        column: u32,
    },
    #[error(
        "web JSON message path conflict for locale {locale} namespace {namespace} path {message_path}"
    )]
    MessagePathConflict {
        locale: String,
        namespace: String,
        message_path: String,
    },
}

#[derive(Debug, Error)]
pub enum PlainJsonWebBuildError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Config(#[from] WebTargetConfigError),
    #[error(transparent)]
    Catalogs(#[from] ProjectCatalogError),
    #[error(transparent)]
    Export(#[from] WebJsonExportError),
    #[error(transparent)]
    Metadata(#[from] WebMessageMetadataError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Artifacts(#[from] WebArtifactError),
    #[error(transparent)]
    DtoTypes(#[from] WebDtoTypeArtifactError),
    #[error("missing web target {0}")]
    MissingTarget(String),
    #[error("web target {target} must use plain-json mode")]
    UnsupportedTargetMode { target: String },
}

struct PendingMessageFile {
    locale: String,
    namespace: String,
    path: PathBuf,
    relative_path: PathBuf,
    bytes: Vec<u8>,
}

struct PendingExport {
    files: Vec<PendingMessageFile>,
    manifest_path: PathBuf,
    manifest_bytes: Vec<u8>,
    messages_dir: PathBuf,
    default_locale: String,
    supported_locales: Vec<String>,
    namespaces: Vec<String>,
    message_metadata: Vec<WebMessageMetadata>,
}

pub fn export_web_json(
    options: &WebJsonExportOptions,
) -> Result<WebJsonExportOutput, WebJsonExportError> {
    let project = ProjectLayout::load_or_default(options.config_path())?;
    let mut rerun_paths = BTreeSet::from([options.config_path().to_path_buf()]);
    let loaded_catalogs = crate::project_catalogs::load_project_catalogs(&project)?;
    rerun_paths.extend(loaded_catalogs.rerun_if_changed_paths().iter().cloned());
    let pending = build_pending_export(
        project.config().default_locale.as_str(),
        loaded_catalogs.catalogs(),
        options.out_dir(),
        options.mode(),
    )?;

    for file in &pending.files {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&file.path, &file.bytes)?;
    }
    fs::write(&pending.manifest_path, &pending.manifest_bytes)?;

    Ok(WebJsonExportOutput {
        out_dir: options.out_dir().to_path_buf(),
        messages_dir: pending.messages_dir,
        manifest_path: pending.manifest_path,
        message_files: pending
            .files
            .into_iter()
            .map(|file| WebJsonMessageFile {
                locale: file.locale,
                namespace: file.namespace,
                path: file.path,
            })
            .collect(),
        rerun_if_changed_paths: rerun_paths.into_iter().collect(),
        default_locale: pending.default_locale,
        supported_locales: pending.supported_locales,
        namespaces: pending.namespaces,
        mode: options.mode(),
        message_metadata: pending.message_metadata,
    })
}

pub fn build_plain_json_web_target(
    options: &PlainJsonWebBuildOptions,
) -> Result<PlainJsonWebBuildOutput, PlainJsonWebBuildError> {
    let project = ProjectLayout::load_or_default(options.config_path())?;
    project.config().validate_web_targets()?;
    let target = project
        .config()
        .web
        .target(options.target_name())
        .ok_or_else(|| PlainJsonWebBuildError::MissingTarget(options.target_name().to_owned()))?;
    if target.mode != WebTargetMode::PlainJson {
        return Err(PlainJsonWebBuildError::UnsupportedTargetMode {
            target: target.name.clone(),
        });
    }

    let loaded_catalogs = crate::project_catalogs::load_project_catalogs(&project)?;
    let pending = build_plain_pending_export_for_target(
        project.config().default_locale.as_str(),
        loaded_catalogs.catalogs(),
        target,
    )?;
    let mut artifact_files = pending
        .files
        .iter()
        .map(|file| WebArtifactFile::new(&file.relative_path, file.bytes.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    artifact_files.push(WebArtifactFile::new("metadata.json", {
        let mut bytes = serde_json::to_vec_pretty(&pending.message_metadata)?;
        bytes.push(b'\n');
        bytes
    })?);
    artifact_files.push(WebArtifactFile::new(
        "types.ts",
        render_plain_typescript_types(
            &pending.default_locale,
            &pending.supported_locales,
            &pending.namespaces,
            &pending.message_metadata,
        )
        .into_bytes(),
    )?);
    artifact_files.push(WebArtifactFile::new(
        "loaders.ts",
        render_plain_typescript_loaders(&pending.supported_locales, &pending.namespaces)
            .into_bytes(),
    )?);
    artifact_files.push(WebArtifactFile::new(
        "index.ts",
        b"export * from \"./types\";\nexport * from \"./loaders\";\n".to_vec(),
    )?);
    if options.dto_types() {
        append_web_dto_artifacts(&mut artifact_files)?;
    }

    let out_dir = options
        .out_dir_override()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_config_relative_path(options.config_path(), &target.out_dir));
    let artifact_output = write_web_artifacts(
        &WebArtifactWriteOptions::new(out_dir)
            .with_stale_policy(target.stale_policy.unwrap_or(WebStalePolicy::Fail))
            .with_check(options.check()),
        &WebArtifactManifestInput {
            target: target.name.clone(),
            mode: target.mode,
            framework: target.framework,
            default_locale: pending.default_locale.clone(),
            supported_locales: pending.supported_locales.clone(),
            namespaces: pending.namespaces.clone(),
        },
        artifact_files,
    )?;

    Ok(PlainJsonWebBuildOutput {
        artifact_output,
        default_locale: pending.default_locale,
        supported_locales: pending.supported_locales,
        namespaces: pending.namespaces,
        message_metadata: pending.message_metadata,
    })
}

fn build_pending_export(
    default_locale: &str,
    catalogs: &BTreeMap<String, ProjectCatalog>,
    out_dir: &Path,
    mode: WebJsonMode,
) -> Result<PendingExport, WebJsonExportError> {
    match mode {
        WebJsonMode::Plain => build_plain_pending_export(default_locale, catalogs, out_dir),
    }
}

fn build_plain_pending_export(
    default_locale: &str,
    catalogs: &BTreeMap<String, ProjectCatalog>,
    out_dir: &Path,
) -> Result<PendingExport, WebJsonExportError> {
    build_plain_pending_export_inner(default_locale, catalogs, out_dir, None, None)
}

fn build_plain_pending_export_for_target(
    default_locale: &str,
    catalogs: &BTreeMap<String, ProjectCatalog>,
    target: &WebTargetConfig,
) -> Result<PendingExport, WebJsonExportError> {
    build_plain_pending_export_inner(
        default_locale,
        catalogs,
        Path::new(""),
        (!target.locales.is_empty()).then_some(target.locales.as_slice()),
        (!target.namespaces.is_empty()).then_some(target.namespaces.as_slice()),
    )
}

fn build_plain_pending_export_inner(
    default_locale: &str,
    catalogs: &BTreeMap<String, ProjectCatalog>,
    out_dir: &Path,
    locale_filter: Option<&[String]>,
    namespace_filter: Option<&[String]>,
) -> Result<PendingExport, WebJsonExportError> {
    let supported_locales = catalogs.keys().cloned().collect::<Vec<_>>();
    let default_catalog = catalogs
        .get(default_locale)
        .ok_or_else(|| ProjectCatalogError::MissingDefaultLocale(default_locale.to_owned()))?;
    let message_metadata =
        collect_web_message_metadata(default_locale, default_catalog, namespace_filter)?;
    let mut namespaces = BTreeSet::new();
    let mut namespace_maps = BTreeMap::<(String, String), Map<String, Value>>::new();

    for (locale, catalog) in catalogs {
        if locale_filter.is_some_and(|values| !values.contains(locale)) {
            continue;
        }
        for message in catalog.values() {
            if namespace_filter.is_some_and(|values| !values.contains(&message.namespace)) {
                continue;
            }
            let text = plain_text_message(locale, message)?;
            namespaces.insert(message.namespace.clone());
            let namespace_map = namespace_maps
                .entry((locale.clone(), message.namespace.clone()))
                .or_default();
            insert_nested_message(namespace_map, locale, message, text)?;
        }
    }

    let namespaces = namespaces.into_iter().collect::<Vec<_>>();
    let supported_locales = match locale_filter {
        Some(values) => values.to_vec(),
        None => supported_locales,
    };
    let messages_dir = out_dir.join(MESSAGES_DIR);
    let mut files = Vec::new();
    for ((locale, namespace), map) in namespace_maps {
        let relative_path = Path::new(MESSAGES_DIR)
            .join(&locale)
            .join(format!("{namespace}.json"));
        let path = out_dir.join(&relative_path);
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(map))?;
        bytes.push(b'\n');
        files.push(PendingMessageFile {
            locale,
            namespace,
            path,
            relative_path,
            bytes,
        });
    }

    let manifest_path = out_dir.join(TYPESCRIPT_MANIFEST_FILE);
    let manifest_bytes =
        render_typescript_manifest(default_locale, &supported_locales, &namespaces).into_bytes();

    Ok(PendingExport {
        files,
        manifest_path,
        manifest_bytes,
        messages_dir,
        default_locale: default_locale.to_owned(),
        supported_locales,
        namespaces,
        message_metadata,
    })
}

pub(crate) fn plain_text_message(
    locale: &str,
    message: &ProjectCatalogMessage,
) -> Result<String, WebJsonExportError> {
    let parsed = parse_message(&message.value).map_err(|error| WebJsonExportError::Parse {
        locale: locale.to_owned(),
        key: message.qualified_key.clone(),
        message: error.message,
        line: error.span.line,
        column: error.span.column,
    })?;
    plain_text_from_ast(locale, message, &parsed)
}

fn plain_text_from_ast(
    locale: &str,
    source: &ProjectCatalogMessage,
    message: &Message,
) -> Result<String, WebJsonExportError> {
    let mut output = String::new();
    for segment in &message.segments {
        match segment {
            Segment::Text { value, .. } => output.push_str(value),
            Segment::Expr(Expr::Variable(var)) => {
                let kind = if var.formatter.is_some() {
                    WebJsonUnsupportedKind::Formatter
                } else {
                    WebJsonUnsupportedKind::Variable
                };
                return Err(WebJsonExportError::UnsupportedMessage {
                    locale: locale.to_owned(),
                    key: source.qualified_key.clone(),
                    kind,
                    line: var.span.line,
                    column: var.span.column,
                });
            }
            Segment::Expr(Expr::Select(select)) => {
                let kind = match select.kind {
                    SelectKind::Select => WebJsonUnsupportedKind::Select,
                    SelectKind::Plural => WebJsonUnsupportedKind::Plural,
                };
                return Err(WebJsonExportError::UnsupportedMessage {
                    locale: locale.to_owned(),
                    key: source.qualified_key.clone(),
                    kind,
                    line: select.span.line,
                    column: select.span.column,
                });
            }
        }
    }
    Ok(output)
}

pub(crate) fn insert_nested_message(
    map: &mut Map<String, Value>,
    locale: &str,
    message: &ProjectCatalogMessage,
    text: String,
) -> Result<(), WebJsonExportError> {
    let parts = message.message_path.split('.').collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) {
        return Err(path_conflict(locale, message));
    }
    insert_nested_parts(map, &parts, text, locale, message)
}

fn insert_nested_parts(
    map: &mut Map<String, Value>,
    parts: &[&str],
    text: String,
    locale: &str,
    message: &ProjectCatalogMessage,
) -> Result<(), WebJsonExportError> {
    let Some((first, rest)) = parts.split_first() else {
        return Err(path_conflict(locale, message));
    };
    if rest.is_empty() {
        if map
            .insert((*first).to_owned(), Value::String(text))
            .is_some()
        {
            return Err(path_conflict(locale, message));
        }
        return Ok(());
    }

    let value = map
        .entry((*first).to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    match value {
        Value::Object(child) => insert_nested_parts(child, rest, text, locale, message),
        _ => Err(path_conflict(locale, message)),
    }
}

fn path_conflict(locale: &str, message: &ProjectCatalogMessage) -> WebJsonExportError {
    WebJsonExportError::MessagePathConflict {
        locale: locale.to_owned(),
        namespace: message.namespace.clone(),
        message_path: message.message_path.clone(),
    }
}

fn render_typescript_manifest(
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
) -> String {
    let supported_locale_values = render_string_array(supported_locales);
    let namespace_values = render_string_array(namespaces);
    let mut loader_entries = Vec::new();
    for locale in supported_locales {
        for namespace in namespaces {
            let import_path = format!("./{MESSAGES_DIR}/{locale}/{namespace}.json");
            loader_entries.push(format!(
                "  {{\n    locale: {},\n    key: {},\n    loader: async () => (await import({})).default,\n  }}",
                json_string(locale),
                json_string(namespace),
                json_string(&import_path)
            ));
        }
    }
    let loaders = if loader_entries.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", loader_entries.join(",\n"))
    };

    format!(
        "export const DEFAULT_LOCALE = {} as const;\nexport const SUPPORTED_LOCALES = [{}] as const;\nexport const MESSAGE_NAMESPACES = [{}] as const;\nexport const MESSAGE_LOADERS = [{}] as const;\n",
        json_string(default_locale),
        supported_locale_values,
        namespace_values,
        loaders
    )
}

fn render_plain_typescript_types(
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
    message_metadata: &[WebMessageMetadata],
) -> String {
    format!(
        "{}export type PlainJsonMessages = Record<string, unknown>;\n",
        render_web_typescript_metadata_types(
            default_locale,
            supported_locales,
            namespaces,
            message_metadata
        )
    )
}

fn render_plain_typescript_loaders(supported_locales: &[String], namespaces: &[String]) -> String {
    let mut loader_entries = Vec::new();
    for locale in supported_locales {
        for namespace in namespaces {
            let import_path = format!("./{MESSAGES_DIR}/{locale}/{namespace}.json");
            loader_entries.push(format!(
                "  {{\n    locale: {},\n    namespace: {},\n    loader: async () => (await import({})).default as PlainJsonMessages,\n  }}",
                json_string(locale),
                json_string(namespace),
                json_string(&import_path)
            ));
        }
    }
    let loaders = if loader_entries.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", loader_entries.join(",\n"))
    };

    format!(
        "import type {{ Locale, Namespace, PlainJsonMessages }} from \"./types\";\n\nexport const MESSAGE_LOADERS = [{}] as const;\n\nexport async function loadMessages(locale: Locale, namespace: Namespace): Promise<PlainJsonMessages> {{\n  const entry = MESSAGE_LOADERS.find((loader) => loader.locale === locale && loader.namespace === namespace);\n  if (!entry) {{\n    throw new Error(`missing MF2 messages for ${{locale}}/${{namespace}}`);\n  }}\n  return entry.loader();\n}}\n",
        loaders,
    )
}

fn render_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}

#[cfg(test)]
mod tests {
    use super::{
        PlainJsonWebBuildOptions, WebJsonExportError, WebJsonExportOptions, WebJsonMode,
        WebJsonUnsupportedKind, build_plain_json_web_target, export_web_json,
    };
    use crate::web::WEB_ARTIFACT_MANIFEST_FILE;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_web_json_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_config(root: &Path) -> PathBuf {
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
        )
        .expect("config");
        config_path
    }

    fn write_web_config(root: &Path) -> PathBuf {
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            "version = 1
default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n\n[[web.targets]]\nname = \"docs\"\nmode = \"plain-json\"\nframework = \"docusaurus\"\nout_dir = \"web-generated\"\nlocales = [\"en\", \"es\"]\nnamespaces = [\"common\"]\nstale_policy = \"fail\"\n",
        )
        .expect("config");
        config_path
    }

    fn write_locale(root: &Path, locale: &str, common_title: &str, product_name: &str) {
        let locale_dir = root.join("locales").join(locale);
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(
            locale_dir.join("common.json"),
            format!(r#"{{"home":{{"title":{common_title:?}}}}}"#),
        )
        .expect("common");
        fs::write(
            locale_dir.join("products.mf2"),
            format!("key.coffee.name = {product_name}\n"),
        )
        .expect("products");
    }

    #[test]
    fn exports_plain_web_json_and_typescript_manifest() {
        let root = temp_dir("plain");
        write_locale(&root, "en", "Hi", "Coffee");
        write_locale(&root, "es", "Hola", "Cafe");
        let config_path = write_config(&root);

        let out_dir = root.join("web-json");
        let output =
            export_web_json(&WebJsonExportOptions::new(&config_path, &out_dir)).expect("export");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(
            output.namespaces(),
            &["common".to_string(), "products".to_string()]
        );
        assert_eq!(output.mode(), WebJsonMode::Plain);
        assert_eq!(output.message_files().len(), 4);
        assert_eq!(output.message_metadata().len(), 2);
        assert!(output.rerun_if_changed_paths().contains(&config_path));

        let common_json =
            fs::read_to_string(out_dir.join("messages/en/common.json")).expect("common json");
        let common: serde_json::Value = serde_json::from_str(&common_json).expect("json");
        assert_eq!(common["home"]["title"], "Hi");

        let products_json =
            fs::read_to_string(out_dir.join("messages/en/products.json")).expect("products json");
        let products: serde_json::Value = serde_json::from_str(&products_json).expect("json");
        assert_eq!(products["key"]["coffee"]["name"], "Coffee");

        let manifest = fs::read_to_string(output.manifest_path()).expect("manifest");
        assert!(manifest.contains("export const DEFAULT_LOCALE = \"en\" as const;"));
        assert!(manifest.contains("export const SUPPORTED_LOCALES = [\"en\", \"es\"] as const;"));
        assert!(manifest.contains("export const MESSAGE_NAMESPACES = [\"common\", \"products\"]"));
        assert!(manifest.contains("import(\"./messages/en/common.json\")"));
        assert!(manifest.contains("import(\"./messages/es/products.json\")"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn builds_plain_json_web_target_artifacts() {
        let root = temp_dir("plain_target");
        write_locale(&root, "en", "Hi", "Coffee");
        write_locale(&root, "es", "Hola", "Cafe");
        let config_path = write_web_config(&root);

        let output =
            build_plain_json_web_target(&PlainJsonWebBuildOptions::new(&config_path, "docs"))
                .expect("build");
        let out_dir = root.join("web-generated");

        assert_eq!(output.default_locale(), "en");
        assert_eq!(
            output.supported_locales(),
            &["en".to_string(), "es".to_string()]
        );
        assert_eq!(output.namespaces(), &["common".to_string()]);
        assert_eq!(output.message_metadata().len(), 1);
        assert_eq!(output.artifact_output().manifest().target, "docs");
        assert!(out_dir.join(WEB_ARTIFACT_MANIFEST_FILE).exists());
        assert!(out_dir.join("metadata.json").exists());
        assert!(out_dir.join("messages/en/common.json").exists());
        assert!(!out_dir.join("messages/en/products.json").exists());

        let types = fs::read_to_string(out_dir.join("types.ts")).expect("types");
        assert!(types.contains("export type Locale"));
        assert!(types.contains("export type Namespace"));
        assert!(types.contains("export type MessageId"));
        assert!(types.contains("\"common.home.title\""));
        assert!(types.contains("export type ArgsFor"));

        let loaders = fs::read_to_string(out_dir.join("loaders.ts")).expect("loaders");
        assert!(loaders.contains("loadMessages"));
        assert!(loaders.contains("messages/en/common.json"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn plain_mode_rejects_variables_before_writing_output() {
        let root = temp_dir("variable");
        let locale_dir = root.join("locales").join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(
            locale_dir.join("common.json"),
            r#"{"home":{"title":"Hi { $name }"}}"#,
        )
        .expect("common");
        let config_path = write_config(&root);
        let out_dir = root.join("web-json");

        let err = export_web_json(&WebJsonExportOptions::new(&config_path, &out_dir))
            .expect_err("variable should fail");

        assert!(matches!(
            err,
            WebJsonExportError::UnsupportedMessage {
                locale,
                key,
                kind: WebJsonUnsupportedKind::Variable,
                ..
            } if locale == "en" && key == "common.home.title"
        ));
        assert!(!out_dir.exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn plain_mode_rejects_formatter_select_and_plural_messages() {
        assert_rejects_kind("{ $value :number }", WebJsonUnsupportedKind::Formatter);
        assert_rejects_kind(
            "{ $status -> [ready] {Ready} *[other] {Other} }",
            WebJsonUnsupportedKind::Select,
        );
        assert_rejects_kind(
            "{ $count :plural -> [one] {One} *[other] {Many} }",
            WebJsonUnsupportedKind::Plural,
        );
    }

    fn assert_rejects_kind(source: &str, expected_kind: WebJsonUnsupportedKind) {
        let root = temp_dir("unsupported");
        let locale_dir = root.join("locales").join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(
            locale_dir.join("common.json"),
            format!(r#"{{"home":{{"title":{source:?}}}}}"#),
        )
        .expect("common");
        let config_path = write_config(&root);

        let err = export_web_json(&WebJsonExportOptions::new(
            &config_path,
            root.join("web-json"),
        ))
        .expect_err("unsupported should fail");

        assert!(matches!(
            err,
            WebJsonExportError::UnsupportedMessage { kind, .. } if kind == expected_kind
        ));

        fs::remove_dir_all(&root).ok();
    }
}

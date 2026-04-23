use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde_json::{Map, Value};
use thiserror::Error;

use crate::parser::{Expr, Message, Segment, SelectKind, parse_message};
use crate::project::{ProjectError, ProjectLayout};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogError, ProjectCatalogMessage};

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

struct PendingMessageFile {
    locale: String,
    namespace: String,
    path: PathBuf,
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
    let supported_locales = catalogs.keys().cloned().collect::<Vec<_>>();
    let mut namespaces = BTreeSet::new();
    let mut namespace_maps = BTreeMap::<(String, String), Map<String, Value>>::new();

    for (locale, catalog) in catalogs {
        for message in catalog.values() {
            let text = plain_text_message(locale, message)?;
            namespaces.insert(message.namespace.clone());
            let namespace_map = namespace_maps
                .entry((locale.clone(), message.namespace.clone()))
                .or_default();
            insert_nested_message(namespace_map, locale, message, text)?;
        }
    }

    let namespaces = namespaces.into_iter().collect::<Vec<_>>();
    let messages_dir = out_dir.join(MESSAGES_DIR);
    let mut files = Vec::new();
    for ((locale, namespace), map) in namespace_maps {
        let path = messages_dir.join(&locale).join(format!("{namespace}.json"));
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(map))?;
        bytes.push(b'\n');
        files.push(PendingMessageFile {
            locale,
            namespace,
            path,
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
    })
}

fn plain_text_message(
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

fn insert_nested_message(
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
        WebJsonExportError, WebJsonExportOptions, WebJsonMode, WebJsonUnsupportedKind,
        export_web_json,
    };
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
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n",
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

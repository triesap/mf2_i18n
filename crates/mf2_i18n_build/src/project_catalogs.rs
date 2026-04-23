use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;

use crate::mf2_source::parse_mf2_source;
use crate::project::ProjectLayout;

pub type ProjectCatalog = BTreeMap<String, ProjectCatalogMessage>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCatalogMessage {
    pub namespace: String,
    pub message_path: String,
    pub qualified_key: String,
    pub value: String,
    pub source_path: PathBuf,
    pub line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ProjectCatalogLoad {
    catalogs: BTreeMap<String, ProjectCatalog>,
    rerun_if_changed_paths: BTreeSet<PathBuf>,
}

impl ProjectCatalogLoad {
    pub fn catalogs(&self) -> &BTreeMap<String, ProjectCatalog> {
        &self.catalogs
    }

    pub fn catalog(&self, locale: &str) -> Option<&ProjectCatalog> {
        self.catalogs.get(locale)
    }

    pub fn rerun_if_changed_paths(&self) -> &BTreeSet<PathBuf> {
        &self.rerun_if_changed_paths
    }
}

#[derive(Debug, Error)]
pub enum ProjectCatalogError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error in {path:?}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("project config must declare at least one source dir")]
    NoSourceDirs,
    #[error("at least one locale catalog is required")]
    NoLocaleCatalogs,
    #[error("default locale {0} catalog should exist")]
    MissingDefaultLocale(String),
    #[error("namespace file must have a non-empty stem: {path:?}")]
    EmptyNamespace { path: PathBuf },
    #[error("json message path must not be empty in {path:?}")]
    EmptyJsonMessagePath { path: PathBuf },
    #[error("json message key segment must not be empty in {path:?}")]
    EmptyJsonKeySegment { path: PathBuf },
    #[error("json message leaf at {json_path} in {path:?} must be a string, found {kind}")]
    NonStringJsonLeaf {
        path: PathBuf,
        json_path: String,
        kind: &'static str,
    },
    #[error("source parse error in {path:?} at {line}:{column}: {message}")]
    SourceParse {
        path: PathBuf,
        line: u32,
        column: u32,
        message: String,
    },
    #[error(
        "duplicate i18n message key {key} in locale {locale}: {first_path:?} and {duplicate_path:?}"
    )]
    DuplicateMessageKey {
        key: String,
        locale: String,
        first_path: PathBuf,
        duplicate_path: PathBuf,
    },
    #[error(
        "i18n catalog keys for locale {locale} do not match reference locale {reference_locale}"
    )]
    CatalogKeyMismatch {
        locale: String,
        reference_locale: String,
    },
}

pub fn load_project_catalogs(
    project: &ProjectLayout,
) -> Result<ProjectCatalogLoad, ProjectCatalogError> {
    let source_roots = project.source_roots();
    if source_roots.is_empty() {
        return Err(ProjectCatalogError::NoSourceDirs);
    }

    let mut catalogs = BTreeMap::<String, ProjectCatalog>::new();
    let mut rerun_if_changed_paths = BTreeSet::new();

    for source_root in source_roots {
        rerun_if_changed_paths.insert(source_root.clone());
        let entries = fs::read_dir(&source_root)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let locale = entry.file_name().to_string_lossy().into_owned();
            rerun_if_changed_paths.insert(path.clone());
            load_locale_catalog(&path, &locale, &mut catalogs, &mut rerun_if_changed_paths)?;
        }
    }

    if catalogs.is_empty() {
        return Err(ProjectCatalogError::NoLocaleCatalogs);
    }

    let default_locale = project.config().default_locale.clone();
    if !catalogs.contains_key(&default_locale) {
        return Err(ProjectCatalogError::MissingDefaultLocale(default_locale));
    }

    ensure_catalog_keys_match(&catalogs)?;

    Ok(ProjectCatalogLoad {
        catalogs,
        rerun_if_changed_paths,
    })
}

fn load_locale_catalog(
    locale_dir: &Path,
    locale: &str,
    catalogs: &mut BTreeMap<String, ProjectCatalog>,
    rerun_if_changed_paths: &mut BTreeSet<PathBuf>,
) -> Result<(), ProjectCatalogError> {
    let entries = fs::read_dir(locale_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        match path.extension().and_then(|value| value.to_str()) {
            Some("json") => {
                rerun_if_changed_paths.insert(path.clone());
                load_json_namespace(&path, locale, catalogs)?;
            }
            Some("mf2") => {
                rerun_if_changed_paths.insert(path.clone());
                load_mf2_namespace(&path, locale, catalogs)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn load_json_namespace(
    path: &Path,
    locale: &str,
    catalogs: &mut BTreeMap<String, ProjectCatalog>,
) -> Result<(), ProjectCatalogError> {
    let namespace = namespace_from_path(path)?;
    let raw = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&raw).map_err(|source| ProjectCatalogError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let mut parts = Vec::new();
    flatten_json_namespace(path, locale, &namespace, &value, &mut parts, catalogs)
}

fn flatten_json_namespace(
    path: &Path,
    locale: &str,
    namespace: &str,
    value: &Value,
    parts: &mut Vec<String>,
    catalogs: &mut BTreeMap<String, ProjectCatalog>,
) -> Result<(), ProjectCatalogError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key.is_empty() {
                    return Err(ProjectCatalogError::EmptyJsonKeySegment {
                        path: path.to_path_buf(),
                    });
                }
                parts.push(key.clone());
                flatten_json_namespace(path, locale, namespace, child, parts, catalogs)?;
                parts.pop();
            }
            Ok(())
        }
        Value::String(message) => {
            if parts.is_empty() {
                return Err(ProjectCatalogError::EmptyJsonMessagePath {
                    path: path.to_path_buf(),
                });
            }
            let key = parts.join(".");
            insert_message(
                catalogs,
                locale,
                ProjectCatalogMessage {
                    namespace: namespace.to_owned(),
                    message_path: key.clone(),
                    qualified_key: qualified_key(namespace, &key),
                    value: message.clone(),
                    source_path: path.to_path_buf(),
                    line: None,
                },
            )
        }
        _ => Err(ProjectCatalogError::NonStringJsonLeaf {
            path: path.to_path_buf(),
            json_path: if parts.is_empty() {
                "$".to_owned()
            } else {
                parts.join(".")
            },
            kind: json_kind(value),
        }),
    }
}

fn load_mf2_namespace(
    path: &Path,
    locale: &str,
    catalogs: &mut BTreeMap<String, ProjectCatalog>,
) -> Result<(), ProjectCatalogError> {
    let namespace = namespace_from_path(path)?;
    let raw = fs::read_to_string(path)?;
    let entries = parse_mf2_source(&raw).map_err(|error| ProjectCatalogError::SourceParse {
        path: path.to_path_buf(),
        line: error.line,
        column: error.column,
        message: error.message,
    })?;

    for entry in entries {
        insert_message(
            catalogs,
            locale,
            ProjectCatalogMessage {
                namespace: namespace.clone(),
                message_path: entry.key.clone(),
                qualified_key: qualified_key(&namespace, &entry.key),
                value: entry.value,
                source_path: path.to_path_buf(),
                line: Some(entry.line),
            },
        )?;
    }

    Ok(())
}

fn namespace_from_path(path: &Path) -> Result<String, ProjectCatalogError> {
    let namespace = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .trim();
    if namespace.is_empty() {
        return Err(ProjectCatalogError::EmptyNamespace {
            path: path.to_path_buf(),
        });
    }
    Ok(namespace.to_owned())
}

fn insert_message(
    catalogs: &mut BTreeMap<String, ProjectCatalog>,
    locale: &str,
    message: ProjectCatalogMessage,
) -> Result<(), ProjectCatalogError> {
    let catalog = catalogs.entry(locale.to_owned()).or_default();
    if let Some(previous) = catalog.get(&message.qualified_key) {
        return Err(ProjectCatalogError::DuplicateMessageKey {
            key: message.qualified_key.clone(),
            locale: locale.to_owned(),
            first_path: previous.source_path.clone(),
            duplicate_path: message.source_path,
        });
    }
    catalog.insert(message.qualified_key.clone(), message);
    Ok(())
}

fn ensure_catalog_keys_match(
    catalogs: &BTreeMap<String, ProjectCatalog>,
) -> Result<(), ProjectCatalogError> {
    let Some((reference_locale, reference_catalog)) = catalogs.iter().next() else {
        return Err(ProjectCatalogError::NoLocaleCatalogs);
    };

    let reference_keys = reference_catalog.keys().cloned().collect::<Vec<_>>();
    for (locale, catalog) in catalogs.iter().skip(1) {
        let keys = catalog.keys().cloned().collect::<Vec<_>>();
        if keys != reference_keys {
            return Err(ProjectCatalogError::CatalogKeyMismatch {
                locale: locale.clone(),
                reference_locale: reference_locale.clone(),
            });
        }
    }

    Ok(())
}

fn qualified_key(namespace: &str, key: &str) -> String {
    format!("{namespace}.{key}")
}

fn json_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::{ProjectCatalogError, load_project_catalogs};
    use crate::project::ProjectLayout;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_project_catalogs_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn write_config(root: &Path, default_locale: &str) -> PathBuf {
        let config_path = root.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            format!(
                "default_locale = {default_locale:?}\nsource_dirs = [\"locales\"]\nproject_salt_path = \"id_salt.txt\"\n"
            ),
        )
        .expect("config");
        fs::write(root.join("id_salt.txt"), "salt").expect("salt");
        config_path
    }

    fn load(
        root: &Path,
        default_locale: &str,
    ) -> Result<super::ProjectCatalogLoad, ProjectCatalogError> {
        let config_path = write_config(root, default_locale);
        let project = ProjectLayout::load(&config_path).expect("project");
        load_project_catalogs(&project)
    }

    #[test]
    fn loads_namespace_json_and_mf2_sources() {
        let root = temp_dir("success");
        let en_dir = root.join("locales").join("en");
        let es_dir = root.join("locales").join("es");
        fs::create_dir_all(&en_dir).expect("en");
        fs::create_dir_all(&es_dir).expect("es");

        fs::write(
            en_dir.join("common.json"),
            r#"{"home":{"title":"Hi"},"button":"Go"}"#,
        )
        .expect("en common");
        fs::write(en_dir.join("checkout.mf2"), "title = Checkout").expect("en checkout");
        fs::write(
            es_dir.join("common.json"),
            r#"{"home":{"title":"Hola"},"button":"Ir"}"#,
        )
        .expect("es common");
        fs::write(es_dir.join("checkout.mf2"), "title = Pagar").expect("es checkout");

        let loaded = load(&root, "en").expect("load");
        let en = loaded.catalog("en").expect("en catalog");
        assert_eq!(en["common.home.title"].value, "Hi");
        assert_eq!(en["common.button"].value, "Go");
        assert_eq!(en["checkout.title"].value, "Checkout");
        assert!(
            loaded
                .rerun_if_changed_paths()
                .contains(&en_dir.join("common.json"))
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_duplicate_qualified_keys() {
        let root = temp_dir("duplicate");
        let en_dir = root.join("locales").join("en");
        fs::create_dir_all(&en_dir).expect("en");
        fs::write(en_dir.join("common.json"), r#"{"title":"Hi"}"#).expect("json");
        fs::write(en_dir.join("common.mf2"), "title = Hello").expect("mf2");

        let err = load(&root, "en").expect_err("duplicate");
        assert!(matches!(
            err,
            ProjectCatalogError::DuplicateMessageKey { key, locale, .. }
                if key == "common.title" && locale == "en"
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_mismatched_locale_keys() {
        let root = temp_dir("mismatch");
        let en_dir = root.join("locales").join("en");
        let es_dir = root.join("locales").join("es");
        fs::create_dir_all(&en_dir).expect("en");
        fs::create_dir_all(&es_dir).expect("es");
        fs::write(en_dir.join("common.json"), r#"{"title":"Hi"}"#).expect("en json");
        fs::write(es_dir.join("common.json"), r#"{"subtitle":"Hola"}"#).expect("es json");

        let err = load(&root, "en").expect_err("mismatch");
        assert!(matches!(
            err,
            ProjectCatalogError::CatalogKeyMismatch { locale, reference_locale }
                if locale == "es" && reference_locale == "en"
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_missing_default_locale() {
        let root = temp_dir("missing_default");
        let en_dir = root.join("locales").join("en");
        fs::create_dir_all(&en_dir).expect("en");
        fs::write(en_dir.join("common.json"), r#"{"title":"Hi"}"#).expect("json");

        let err = load(&root, "fr").expect_err("missing default");
        assert!(matches!(
            err,
            ProjectCatalogError::MissingDefaultLocale(locale) if locale == "fr"
        ));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_non_string_json_leaves() {
        let root = temp_dir("non_string");
        let en_dir = root.join("locales").join("en");
        fs::create_dir_all(&en_dir).expect("en");
        fs::write(en_dir.join("common.json"), r#"{"title":1}"#).expect("json");

        let err = load(&root, "en").expect_err("non string");
        assert!(matches!(
            err,
            ProjectCatalogError::NonStringJsonLeaf { json_path, kind, .. }
                if json_path == "title" && kind == "number"
        ));

        fs::remove_dir_all(&root).ok();
    }
}

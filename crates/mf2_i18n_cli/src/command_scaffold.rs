use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_i18n_build::{
    ProjectCatalogError, ProjectCatalogLoadOptions, ProjectCatalogMessage, ProjectLayout,
    SourceConfig, SourceLayout, load_project_catalogs_with_options,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScaffoldCommandError {
    #[error(transparent)]
    Project(#[from] mf2_i18n_build::ProjectError),
    #[error(transparent)]
    Catalog(#[from] ProjectCatalogError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("source {0} does not exist")]
    UnknownSource(String),
    #[error("locale must not be empty")]
    EmptyLocale,
}

#[derive(Debug, Clone)]
pub struct ScaffoldMissingOptions {
    pub config_path: PathBuf,
    pub source_name: String,
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldMissingReport {
    pub source_name: String,
    pub locale: String,
    pub missing_count: usize,
    pub written_paths: Vec<PathBuf>,
}

pub fn run_scaffold_missing(
    options: &ScaffoldMissingOptions,
) -> Result<ScaffoldMissingReport, ScaffoldCommandError> {
    if options.locale.trim().is_empty() {
        return Err(ScaffoldCommandError::EmptyLocale);
    }
    let project = ProjectLayout::load(&options.config_path)?;
    let source = project
        .config()
        .sources
        .iter()
        .find(|source| source.name == options.source_name)
        .ok_or_else(|| ScaffoldCommandError::UnknownSource(options.source_name.clone()))?;
    let loaded = load_project_catalogs_with_options(
        &project,
        &ProjectCatalogLoadOptions::default().allow_key_mismatch(),
    )?;
    let default_catalog = loaded
        .catalog(&project.config().default_locale)
        .ok_or_else(|| {
            ProjectCatalogError::MissingDefaultLocale(project.config().default_locale.clone())
        })?;
    let target_catalog = loaded.catalog(&options.locale);
    let source_root = project.resolve_path(&source.root);
    let missing_messages = default_catalog
        .values()
        .filter(|message| message.source_path.starts_with(&source_root))
        .filter(|message| {
            target_catalog
                .map(|catalog| !catalog.contains_key(&message.qualified_key))
                .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();
    let written_paths =
        write_missing_messages(source, &source_root, &options.locale, &missing_messages)?;

    Ok(ScaffoldMissingReport {
        source_name: options.source_name.clone(),
        locale: options.locale.clone(),
        missing_count: missing_messages.len(),
        written_paths,
    })
}

fn write_missing_messages(
    source: &SourceConfig,
    source_root: &Path,
    locale: &str,
    messages: &[ProjectCatalogMessage],
) -> Result<Vec<PathBuf>, ScaffoldCommandError> {
    let mut entries_by_path = BTreeMap::<PathBuf, Vec<(&str, &str)>>::new();
    for message in messages {
        let (path, key) = missing_message_path_and_key(source.layout, source_root, locale, message);
        entries_by_path
            .entry(path)
            .or_default()
            .push((key, message.value.as_str()));
    }

    let mut written_paths = Vec::new();
    for (path, entries) in entries_by_path {
        append_mf2_entries(&path, &entries)?;
        written_paths.push(path);
    }
    Ok(written_paths)
}

fn missing_message_path_and_key<'a>(
    layout: SourceLayout,
    source_root: &Path,
    locale: &str,
    message: &'a ProjectCatalogMessage,
) -> (PathBuf, &'a str) {
    match layout {
        SourceLayout::LocaleNamespace | SourceLayout::DocusaurusDocs => (
            source_root
                .join(locale)
                .join(format!("{}.mf2", message.namespace)),
            message.message_path.as_str(),
        ),
        SourceLayout::NamespaceLocale => (
            source_root
                .join(&message.namespace)
                .join(format!("{locale}.mf2")),
            message.message_path.as_str(),
        ),
        SourceLayout::SingleLocaleFile => (
            source_root.join(format!("{locale}.mf2")),
            message.qualified_key.as_str(),
        ),
    }
}

fn append_mf2_entries(path: &Path, entries: &[(&str, &str)]) -> Result<(), ScaffoldCommandError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut contents = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if !contents.is_empty() {
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        if !contents.ends_with("\n\n") {
            contents.push('\n');
        }
    }
    for (key, value) in entries {
        contents.push_str(key);
        contents.push_str(" = ");
        contents.push_str(value);
        contents.push_str("\n\n");
    }
    fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ScaffoldCommandError, ScaffoldMissingOptions, run_scaffold_missing};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_scaffold_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn writes_missing_locale_namespace_entries() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
namespace_glob = "*.mf2"
"#,
        )
        .expect("config");
        fs::create_dir_all(dir.join("locales/en")).expect("locales");
        fs::write(
            dir.join("locales/en/homepage.mf2"),
            "title = Radroots\n\nbody = Collaborative farming\n",
        )
        .expect("source");

        let report = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "site".to_string(),
            locale: "es".to_string(),
        })
        .expect("scaffold");

        assert_eq!(report.source_name, "site");
        assert_eq!(report.locale, "es");
        assert_eq!(report.missing_count, 2);
        assert_eq!(
            report.written_paths,
            vec![dir.join("locales/es/homepage.mf2")]
        );
        let output = fs::read_to_string(dir.join("locales/es/homepage.mf2")).expect("output");
        assert!(output.contains("title = Radroots\n\n"));
        assert!(output.contains("body = Collaborative farming\n\n"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_only_missing_target_messages_and_is_idempotent() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en", "es"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
"#,
        )
        .expect("config");
        fs::create_dir_all(dir.join("locales/en")).expect("en");
        fs::create_dir_all(dir.join("locales/es")).expect("es");
        fs::write(
            dir.join("locales/en/homepage.mf2"),
            "title = Radroots\n\nbody = Collaborative farming\n",
        )
        .expect("en");
        fs::write(dir.join("locales/es/homepage.mf2"), "title = Radroots\n").expect("es");

        let first = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path: config_path.clone(),
            source_name: "site".to_string(),
            locale: "es".to_string(),
        })
        .expect("first");
        let second = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "site".to_string(),
            locale: "es".to_string(),
        })
        .expect("second");

        assert_eq!(first.missing_count, 1);
        assert_eq!(second.missing_count, 0);
        assert!(second.written_paths.is_empty());
        let output = fs::read_to_string(dir.join("locales/es/homepage.mf2")).expect("output");
        assert_eq!(output.matches("body = Collaborative farming").count(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_namespace_locale_entries() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "namespace-locale"
root = "translations"
"#,
        )
        .expect("config");
        fs::create_dir_all(dir.join("translations/homepage")).expect("namespace");
        fs::write(
            dir.join("translations/homepage/en.mf2"),
            "title = Radroots\n",
        )
        .expect("en");

        let report = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "site".to_string(),
            locale: "fr".to_string(),
        })
        .expect("scaffold");

        assert_eq!(report.missing_count, 1);
        let output = fs::read_to_string(dir.join("translations/homepage/fr.mf2")).expect("output");
        assert_eq!(output, "title = Radroots\n\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_single_locale_file_entries() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "single-locale-file"
root = "locales"
"#,
        )
        .expect("config");
        fs::create_dir_all(dir.join("locales")).expect("locales");
        fs::write(dir.join("locales/en.mf2"), "homepage.title = Radroots\n").expect("en");

        let report = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "site".to_string(),
            locale: "fr".to_string(),
        })
        .expect("scaffold");

        assert_eq!(report.missing_count, 1);
        let output = fs::read_to_string(dir.join("locales/fr.mf2")).expect("output");
        assert_eq!(output, "homepage.title = Radroots\n\n");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_missing_source() {
        let dir = temp_dir();
        let config_path = dir.join("mf2_i18n.toml");
        fs::write(
            &config_path,
            r#"
version = 1
default_locale = "en"
locales = ["en"]

[[sources]]
name = "site"
layout = "locale-namespace"
root = "locales"
"#,
        )
        .expect("config");

        let err = run_scaffold_missing(&ScaffoldMissingOptions {
            config_path,
            source_name: "docs".to_string(),
            locale: "es".to_string(),
        })
        .expect_err("source");

        assert!(matches!(
            err,
            ScaffoldCommandError::UnknownSource(source) if source == "docs"
        ));

        fs::remove_dir_all(&dir).ok();
    }
}

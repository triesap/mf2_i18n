use std::collections::BTreeMap;
use std::path::PathBuf;

use mfs_i18n_build::catalog_reader::{CatalogReadError, load_catalog};
use mfs_i18n_build::diagnostic::Diagnostic;
use mfs_i18n_build::locale_sources::{LocaleBundle, LocaleSourceError, load_locales};
use mfs_i18n_build::model::MessageSpec;
use mfs_i18n_build::parser::parse_message;
use mfs_i18n_build::project::{ProjectError, ProjectLayout};
use mfs_i18n_build::validator::validate_message;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidateCommandError {
    #[error(transparent)]
    Project(#[from] ProjectError),
    #[error(transparent)]
    Catalog(#[from] CatalogReadError),
    #[error(transparent)]
    Source(#[from] LocaleSourceError),
    #[error("validation failed with {0} diagnostics")]
    Failed(usize),
}

#[derive(Debug, Clone)]
pub struct ValidateOptions {
    pub catalog_path: PathBuf,
    pub id_map_hash_path: PathBuf,
    pub config_path: PathBuf,
}

pub fn run_validate(options: &ValidateOptions) -> Result<Vec<Diagnostic>, ValidateCommandError> {
    let project = ProjectLayout::load_or_default(&options.config_path)?;
    let bundle = load_catalog(&options.catalog_path, &options.id_map_hash_path)?;
    let locales = load_locales(&project.source_roots())?;

    let mut diagnostics = Vec::new();
    for locale in locales {
        diagnostics.extend(validate_locale(&locale, &bundle.message_specs));
    }

    if diagnostics.is_empty() {
        Ok(diagnostics)
    } else {
        Err(ValidateCommandError::Failed(diagnostics.len()))
    }
}

fn validate_locale(
    locale: &LocaleBundle,
    specs: &BTreeMap<String, MessageSpec>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (key, spec) in specs {
        if !locale.messages.contains_key(key) {
            diagnostics.push(Diagnostic::new("MF2E100", "missing key").with_span(
                format!("locale:{}", locale.locale),
                1,
                1,
            ));
        } else {
            if let Some(entry) = locale.messages.get(key) {
                match parse_message(&entry.value) {
                    Ok(message) => {
                        for mut diag in validate_message(&message, spec) {
                            let line = entry.line + diag.line.unwrap_or(1) - 1;
                            let column = diag.column.unwrap_or(1);
                            diag.file = Some(entry.file.clone());
                            diag.line = Some(line);
                            diag.column = Some(column);
                            diagnostics.push(diag);
                        }
                    }
                    Err(err) => {
                        diagnostics.push(
                            Diagnostic::new("MF2E001", format!("parse error: {}", err.message))
                                .with_span(entry.file.clone(), entry.line, 1),
                        );
                    }
                }
            }
        }
    }

    for (key, entry) in &locale.messages {
        if !specs.contains_key(key) {
            diagnostics.push(Diagnostic::new("MF2E101", "unknown key").with_span(
                entry.file.clone(),
                entry.line,
                1,
            ));
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::{ValidateOptions, run_validate};
    use mfs_i18n_build::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use mfs_i18n_build::model::{ArgSpec, ArgType};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mfs_i18n_validate_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn reports_missing_keys() {
        let dir = temp_dir();
        let locale_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(locale_dir.join("messages.mf2"), "home.title = Hi").expect("write");

        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![
                CatalogMessage {
                    key: "home.title".to_string(),
                    id: 1,
                    args: vec![],
                    features: CatalogFeatures::default(),
                    source_refs: None,
                },
                CatalogMessage {
                    key: "home.subtitle".to_string(),
                    id: 2,
                    args: vec![ArgSpec {
                        name: "name".to_string(),
                        arg_type: ArgType::String,
                        required: true,
                    }],
                    features: CatalogFeatures::default(),
                    source_refs: None,
                },
            ],
        };

        let catalog_path = dir.join("i18n.catalog.json");
        fs::write(&catalog_path, serde_json::to_string(&catalog).unwrap()).expect("catalog");
        let hash_path = dir.join("id_map_hash");
        fs::write(
            &hash_path,
            "sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("hash");

        let config_path = dir.join("mfs_i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"tools/id_salt.txt\"",
        )
        .expect("config");

        let options = ValidateOptions {
            catalog_path,
            id_map_hash_path: hash_path,
            config_path,
        };
        let err = run_validate(&options).expect_err("validate should fail");
        match err {
            super::ValidateCommandError::Failed(count) => assert!(count > 0),
            _ => panic!("unexpected error"),
        }

        fs::remove_dir_all(&dir).ok();
    }
}

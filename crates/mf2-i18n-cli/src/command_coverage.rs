use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::catalog_reader::{CatalogReadError, load_catalog};
use crate::config::load_config_or_default;
use crate::error::CliError;
use crate::locale_sources::{LocaleSourceError, load_locales};

#[derive(Debug, Error)]
pub enum CoverageCommandError {
    #[error("config error: {0}")]
    Config(#[from] CliError),
    #[error(transparent)]
    Catalog(#[from] CatalogReadError),
    #[error(transparent)]
    Sources(#[from] LocaleSourceError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct CoverageOptions {
    pub catalog_path: PathBuf,
    pub id_map_hash_path: PathBuf,
    pub out_path: PathBuf,
    pub config_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct CoverageReport {
    total_messages: usize,
    locales: BTreeMap<String, LocaleCoverage>,
}

#[derive(Debug, Serialize)]
struct LocaleCoverage {
    present: usize,
    missing: usize,
    extra: usize,
    percent: f64,
    missing_keys: Vec<String>,
}

pub fn run_coverage(options: &CoverageOptions) -> Result<(), CoverageCommandError> {
    let config = load_config_or_default(&options.config_path)?;
    let base_dir = options
        .config_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let roots: Vec<PathBuf> = config
        .source_dirs
        .iter()
        .map(|dir| base_dir.join(dir))
        .collect();

    let catalog = load_catalog(&options.catalog_path, &options.id_map_hash_path)?;
    let locales = load_locales(&roots)?;

    let mut specs = BTreeSet::new();
    for key in catalog.message_specs.keys() {
        specs.insert(key.clone());
    }

    let total = specs.len();
    let mut report_locales = BTreeMap::new();

    for locale in locales {
        let mut missing = Vec::new();
        let mut present = 0usize;
        let mut extra = 0usize;
        for key in &specs {
            if locale.messages.contains_key(key) {
                present += 1;
            } else {
                missing.push(key.clone());
            }
        }
        for key in locale.messages.keys() {
            if !specs.contains(key) {
                extra += 1;
            }
        }
        let percent = if total == 0 {
            100.0
        } else {
            (present as f64 / total as f64) * 100.0
        };
        report_locales.insert(
            locale.locale,
            LocaleCoverage {
                present,
                missing: missing.len(),
                extra,
                percent,
                missing_keys: missing,
            },
        );
    }

    let report = CoverageReport {
        total_messages: total,
        locales: report_locales,
    };
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(&options.out_path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CoverageOptions, run_coverage};
    use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use crate::model::{ArgSpec, ArgType};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_{name}_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn writes_coverage_report() {
        let root = temp_dir("coverage_root");
        let locale_dir = root.join("en");
        fs::create_dir_all(&locale_dir).expect("locale");
        fs::write(locale_dir.join("messages.mf2"), "home.title = Hello").expect("write");

        let config_path = root.join("mf2-i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\".\"]\nmicro_locales_registry = \"micro-locales.toml\"\nproject_salt_path = \"tools/id_salt.txt\"\n",
        )
        .expect("write config");

        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 1,
                args: vec![ArgSpec {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: false,
                }],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };
        let catalog_path = root.join("catalog.json");
        fs::write(
            &catalog_path,
            serde_json::to_string_pretty(&catalog).expect("json"),
        )
        .expect("write catalog");
        let hash_path = root.join("id_map_hash");
        fs::write(
            &hash_path,
            "sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("write hash");

        let out_path = root.join("coverage.json");
        let options = CoverageOptions {
            catalog_path,
            id_map_hash_path: hash_path,
            out_path: out_path.clone(),
            config_path,
        };
        run_coverage(&options).expect("run");
        let contents = fs::read_to_string(&out_path).expect("read");
        assert!(contents.contains("\"total_messages\""));
        assert!(contents.contains("\"present\""));

        fs::remove_dir_all(&root).ok();
    }
}

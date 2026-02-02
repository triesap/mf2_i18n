use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::catalog_reader::{load_catalog, CatalogReadError};
use crate::command_validate::{run_validate, ValidateCommandError, ValidateOptions};
use crate::compiler::compile_message;
use crate::config::load_config_or_default;
use crate::locale_sources::{load_locales, LocaleSourceError};
use crate::manifest::{sha256_hex, Manifest, PackEntry};
use crate::micro_locales::{load_micro_locales, MicroLocaleError};
use crate::pack_encode::{encode_pack, PackBuildInput};
use crate::parser::parse_message;

#[derive(Debug, Error)]
pub enum BuildCommandError {
    #[error("config error: {0}")]
    Config(#[from] crate::error::CliError),
    #[error(transparent)]
    Catalog(#[from] CatalogReadError),
    #[error(transparent)]
    Sources(#[from] LocaleSourceError),
    #[error(transparent)]
    MicroLocales(#[from] MicroLocaleError),
    #[error(transparent)]
    Validate(#[from] ValidateCommandError),
    #[error("missing message {0} for locale {1}")]
    MissingMessage(String, String),
    #[error("parse error for {0}: {1}")]
    ParseError(String, String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub catalog_path: PathBuf,
    pub id_map_hash_path: PathBuf,
    pub config_path: PathBuf,
    pub out_dir: PathBuf,
    pub release_id: String,
    pub generated_at: String,
}

pub fn run_build(options: &BuildOptions) -> Result<(), BuildCommandError> {
    let config = load_config_or_default(&options.config_path)?;
    let bundle = load_catalog(&options.catalog_path, &options.id_map_hash_path)?;
    let roots: Vec<PathBuf> = config
        .source_dirs
        .iter()
        .map(|root| resolve_path(&options.config_path, root))
        .collect();

    run_validate(&ValidateOptions {
        catalog_path: options.catalog_path.clone(),
        id_map_hash_path: options.id_map_hash_path.clone(),
        config_path: options.config_path.clone(),
    })?;

    let locales = load_locales(&roots)?;
    let micro_locale_map = load_micro_locales(
        &resolve_path(
            &options.config_path,
            config.micro_locales_registry.as_deref().unwrap_or("micro-locales.toml"),
        ),
    )?;

    fs::create_dir_all(&options.out_dir)?;
    let packs_dir = options.out_dir.join("packs");
    fs::create_dir_all(&packs_dir)?;

    let mut mf2_packs = BTreeMap::new();
    let mut supported_locales = Vec::new();

    for locale in locales {
        let parent = micro_locale_map.get(&locale.locale).cloned();
        let pack_kind = if parent.is_some() {
            mf2_i18n_core::PackKind::Overlay
        } else {
            mf2_i18n_core::PackKind::Base
        };
        let messages = compile_locale_messages(&locale, &bundle.catalog)?;
        let bytes = encode_pack(&PackBuildInput {
            pack_kind,
            id_map_hash: bundle.id_map_hash,
            locale_tag: locale.locale.clone(),
            parent_tag: parent.clone(),
            build_epoch_ms: 0,
            messages,
        });
        let filename = format!("{}.mf2pack", locale.locale);
        let path = packs_dir.join(&filename);
        fs::write(&path, &bytes)?;
        let hash = sha256_hex(&bytes);
        let entry = PackEntry {
            kind: match pack_kind {
                mf2_i18n_core::PackKind::Base => "base".to_string(),
                mf2_i18n_core::PackKind::Overlay => "overlay".to_string(),
                mf2_i18n_core::PackKind::IcuData => "icu_data".to_string(),
            },
            url: format!("packs/{filename}"),
            hash,
            size: bytes.len() as u64,
            content_encoding: "identity".to_string(),
            pack_schema: 0,
            parent,
        };
        mf2_packs.insert(locale.locale.clone(), entry);
        supported_locales.push(locale.locale);
    }

    supported_locales.sort();
    let manifest = Manifest {
        schema: 1,
        release_id: options.release_id.clone(),
        generated_at: options.generated_at.clone(),
        default_locale: config.default_locale,
        supported_locales,
        id_map_hash: format!("sha256:{}", hex::encode(bundle.id_map_hash)),
        mf2_packs,
        icu_packs: None,
        micro_locales: None,
        budgets: None,
        signing: None,
    };

    let manifest_path = options.out_dir.join("manifest.json");
    fs::write(&manifest_path, manifest.to_canonical_bytes())?;
    Ok(())
}

fn compile_locale_messages(
    locale: &crate::locale_sources::LocaleBundle,
    catalog: &crate::catalog::Catalog,
) -> Result<BTreeMap<mf2_i18n_core::MessageId, mf2_i18n_core::BytecodeProgram>, BuildCommandError>
{
    let mut messages = BTreeMap::new();
    for message in &catalog.messages {
        let entry = locale
            .messages
            .get(&message.key)
            .ok_or_else(|| BuildCommandError::MissingMessage(message.key.clone(), locale.locale.clone()))?;
        let parsed = parse_message(&entry.value)
            .map_err(|err| BuildCommandError::ParseError(message.key.clone(), err.message))?;
        let compiled = compile_message(&parsed);
        messages.insert(mf2_i18n_core::MessageId::new(message.id), compiled.program);
    }
    Ok(messages)
}

fn resolve_path(config_path: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        return path;
    }
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(path)
}

#[cfg(test)]
mod tests {
    use super::{run_build, BuildOptions};
    use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_build_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    #[test]
    fn builds_manifest_and_pack() {
        let dir = temp_dir();
        let locales_dir = dir.join("locales").join("en");
        fs::create_dir_all(&locales_dir).expect("locale");
        fs::write(locales_dir.join("messages.mf2"), "home.title = Hi").expect("write");

        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 1,
                args: vec![],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };
        let catalog_path = dir.join("i18n.catalog.json");
        fs::write(&catalog_path, serde_json::to_string(&catalog).unwrap()).expect("catalog");
        let hash_path = dir.join("id_map_hash");
        fs::write(&hash_path, "sha256:000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
            .expect("hash");

        let config_path = dir.join("mf2-i18n.toml");
        fs::write(
            &config_path,
            "default_locale = \"en\"\nsource_dirs = [\"locales\"]\nproject_salt_path = \"tools/id_salt.txt\"",
        )
        .expect("config");

        let out_dir = dir.join("out");
        run_build(&BuildOptions {
            catalog_path,
            id_map_hash_path: hash_path,
            config_path,
            out_dir: out_dir.clone(),
            release_id: "r1".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
        })
        .expect("build");

        assert!(out_dir.join("manifest.json").exists());
        assert!(out_dir.join("packs/en.mf2pack").exists());

        fs::remove_dir_all(&dir).ok();
    }
}

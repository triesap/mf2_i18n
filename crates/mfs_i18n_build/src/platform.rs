use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::{Manifest, PackEntry, sha256_raw};

pub const PLATFORM_BUNDLE_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformBundleManifest {
    pub schema: u32,
    pub runtime_manifest: Manifest,
    pub id_map_path: String,
}

impl PlatformBundleManifest {
    pub fn new(runtime_manifest: Manifest, id_map_path: impl Into<String>) -> Self {
        Self {
            schema: PLATFORM_BUNDLE_SCHEMA,
            runtime_manifest,
            id_map_path: id_map_path.into(),
        }
    }

    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>, PlatformBundleError> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Clone)]
pub struct PlatformBundle {
    manifest: PlatformBundleManifest,
    id_map_json: Vec<u8>,
    id_map_entries: BTreeMap<String, u32>,
    packs: Vec<PlatformPack>,
}

#[derive(Debug, Clone)]
pub struct PlatformPack {
    pub locale: String,
    pub path: PathBuf,
    pub entry: PackEntry,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum PlatformBundleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported platform bundle schema: {0}")]
    UnsupportedSchema(u32),
    #[error("platform bundle paths must be relative to the bundle root: {0}")]
    UnsupportedAbsolutePath(String),
    #[error("platform bundle paths must not escape the bundle root: {0}")]
    ParentTraversal(String),
    #[error("invalid hash format")]
    InvalidHash,
    #[error("id map hash mismatch")]
    IdMapHashMismatch,
    #[error("pack size mismatch for {0}")]
    PackSizeMismatch(String),
    #[error("pack hash mismatch for {0}")]
    PackHashMismatch(String),
}

impl PlatformBundle {
    pub fn load(bundle_manifest_path: &Path) -> Result<Self, PlatformBundleError> {
        let manifest = load_platform_bundle_manifest(bundle_manifest_path)?;
        Self::load_from_manifest(
            bundle_manifest_path
                .parent()
                .unwrap_or_else(|| Path::new(".")),
            manifest,
        )
    }

    pub fn load_from_manifest(
        bundle_root: &Path,
        manifest: PlatformBundleManifest,
    ) -> Result<Self, PlatformBundleError> {
        let id_map_path = resolve_bundle_path(bundle_root, &manifest.id_map_path)?;
        let id_map_json = fs::read(&id_map_path)?;
        let id_map_entries: BTreeMap<String, u32> = serde_json::from_slice(&id_map_json)?;
        let id_map_hash = hash_id_map_entries(&id_map_entries)?;
        let expected_hash = parse_sha256_literal(&manifest.runtime_manifest.id_map_hash)?;
        if id_map_hash != expected_hash {
            return Err(PlatformBundleError::IdMapHashMismatch);
        }

        let mut packs = Vec::new();
        for (locale, entry) in &manifest.runtime_manifest.mf2_packs {
            let path = resolve_bundle_path(bundle_root, &entry.url)?;
            let bytes = fs::read(&path)?;
            if bytes.len() as u64 != entry.size {
                return Err(PlatformBundleError::PackSizeMismatch(locale.clone()));
            }
            let actual_hash = format!("sha256:{}", hex::encode(sha256_raw(&bytes)));
            if actual_hash != entry.hash {
                return Err(PlatformBundleError::PackHashMismatch(locale.clone()));
            }
            packs.push(PlatformPack {
                locale: locale.clone(),
                path,
                entry: entry.clone(),
                bytes,
            });
        }
        packs.sort_by(|left, right| left.locale.cmp(&right.locale));

        Ok(Self {
            manifest,
            id_map_json,
            id_map_entries,
            packs,
        })
    }

    pub fn manifest(&self) -> &PlatformBundleManifest {
        &self.manifest
    }

    pub fn runtime_manifest(&self) -> &Manifest {
        &self.manifest.runtime_manifest
    }

    pub fn id_map_json(&self) -> &[u8] {
        &self.id_map_json
    }

    pub fn id_map_entries(&self) -> &BTreeMap<String, u32> {
        &self.id_map_entries
    }

    pub fn packs(&self) -> &[PlatformPack] {
        &self.packs
    }

    pub fn pack(&self, locale: &str) -> Option<&PlatformPack> {
        self.packs.iter().find(|pack| pack.locale == locale)
    }
}

pub fn write_platform_bundle_manifest(
    path: &Path,
    manifest: &PlatformBundleManifest,
) -> Result<(), PlatformBundleError> {
    fs::write(path, manifest.to_canonical_bytes()?)?;
    Ok(())
}

pub fn load_platform_bundle_manifest(
    path: &Path,
) -> Result<PlatformBundleManifest, PlatformBundleError> {
    let contents = fs::read_to_string(path)?;
    let manifest: PlatformBundleManifest = serde_json::from_str(&contents)?;
    validate_platform_bundle_manifest(&manifest)?;
    Ok(manifest)
}

pub fn derive_id_map_entries_from_catalog(
    catalog: &crate::catalog::Catalog,
) -> BTreeMap<String, u32> {
    let mut entries = BTreeMap::new();
    for message in &catalog.messages {
        entries.insert(message.key.clone(), message.id);
    }
    entries
}

fn hash_id_map_entries(entries: &BTreeMap<String, u32>) -> Result<[u8; 32], PlatformBundleError> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    for (key, id) in entries {
        let len: u32 = key
            .len()
            .try_into()
            .map_err(|_| PlatformBundleError::InvalidHash)?;
        hasher.update(len.to_le_bytes());
        hasher.update(key.as_bytes());
        hasher.update(id.to_le_bytes());
    }
    Ok(hasher.finalize().into())
}

fn validate_platform_bundle_manifest(
    manifest: &PlatformBundleManifest,
) -> Result<(), PlatformBundleError> {
    if manifest.schema != PLATFORM_BUNDLE_SCHEMA {
        return Err(PlatformBundleError::UnsupportedSchema(manifest.schema));
    }
    Ok(())
}

fn resolve_bundle_path(bundle_root: &Path, raw_path: &str) -> Result<PathBuf, PlatformBundleError> {
    let candidate = Path::new(raw_path);
    if candidate.is_absolute() {
        return Err(PlatformBundleError::UnsupportedAbsolutePath(
            raw_path.to_owned(),
        ));
    }

    let mut resolved = PathBuf::from(bundle_root);
    for component in candidate.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(PlatformBundleError::ParentTraversal(raw_path.to_owned()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(PlatformBundleError::UnsupportedAbsolutePath(
                    raw_path.to_owned(),
                ));
            }
        }
    }

    Ok(resolved)
}

fn parse_sha256_literal(value: &str) -> Result<[u8; 32], PlatformBundleError> {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    let bytes = hex::decode(hex).map_err(|_| PlatformBundleError::InvalidHash)?;
    if bytes.len() != 32 {
        return Err(PlatformBundleError::InvalidHash);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{
        PLATFORM_BUNDLE_SCHEMA, PlatformBundle, PlatformBundleError, PlatformBundleManifest,
        derive_id_map_entries_from_catalog, hash_id_map_entries, load_platform_bundle_manifest,
        write_platform_bundle_manifest,
    };
    use crate::catalog::{Catalog, CatalogFeatures, CatalogMessage};
    use crate::compiler::compile_message;
    use crate::manifest::{Manifest, PackEntry};
    use crate::pack_encode::{PackBuildInput, encode_pack};
    use crate::parser::parse_message;
    use mfs_i18n_core::{MessageId, PackKind};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mfs_i18n_platform_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn build_pack_bytes(id_map_hash: [u8; 32], locale_tag: &str, source: &str) -> Vec<u8> {
        let message = parse_message(source).expect("parse");
        let compiled = compile_message(&message).expect("compile");
        let mut messages = BTreeMap::new();
        messages.insert(MessageId::new(0), compiled.program);
        encode_pack(&PackBuildInput {
            pack_kind: PackKind::Base,
            id_map_hash,
            locale_tag: locale_tag.to_owned(),
            parent_tag: None,
            build_epoch_ms: 0,
            messages,
        })
    }

    fn write_valid_bundle_fixture(root: &Path) -> (PathBuf, Vec<u8>) {
        let packs_dir = root.join("packs");
        fs::create_dir_all(&packs_dir).expect("packs");

        let id_map_json = br#"{"home.title":0}"#;
        let id_map_entries: BTreeMap<String, u32> =
            serde_json::from_slice(id_map_json).expect("id map entries");
        let id_map_hash = hash_id_map_entries(&id_map_entries).expect("id map hash");
        let pack_bytes = build_pack_bytes(id_map_hash, "en", "hi");
        fs::write(packs_dir.join("en.mf2pack"), &pack_bytes).expect("pack");
        fs::write(root.join("id-map.json"), id_map_json).expect("id map");

        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: format!(
                    "sha256:{}",
                    hex::encode(crate::manifest::sha256_raw(&pack_bytes))
                ),
                size: pack_bytes.len() as u64,
                content_encoding: "identity".to_string(),
                pack_schema: 0,
                parent: None,
            },
        );
        let bundle_manifest = PlatformBundleManifest::new(
            Manifest {
                schema: 1,
                release_id: "r1".to_string(),
                generated_at: "2026-02-01T00:00:00Z".to_string(),
                default_locale: "en".to_string(),
                supported_locales: vec!["en".to_string()],
                id_map_hash: format!("sha256:{}", hex::encode(id_map_hash)),
                mf2_packs,
                icu_packs: None,
                micro_locales: None,
                budgets: None,
                signing: None,
            },
            "id-map.json",
        );
        let bundle_manifest_path = root.join("platform-bundle.json");
        write_platform_bundle_manifest(&bundle_manifest_path, &bundle_manifest).expect("manifest");
        (bundle_manifest_path, pack_bytes)
    }

    #[test]
    fn derives_id_map_entries_from_catalog() {
        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 7,
                args: vec![],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };

        let entries = derive_id_map_entries_from_catalog(&catalog);
        assert_eq!(entries.get("home.title"), Some(&7));
    }

    #[test]
    fn loads_platform_bundle_from_bundle_manifest() {
        let root = temp_dir();
        let (bundle_manifest_path, pack_bytes) = write_valid_bundle_fixture(&root);

        let bundle = PlatformBundle::load(&bundle_manifest_path).expect("bundle");
        assert_eq!(bundle.runtime_manifest().default_locale, "en");
        assert_eq!(bundle.id_map_entries().get("home.title"), Some(&0));
        assert_eq!(bundle.pack("en").expect("pack").bytes, pack_bytes);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_unsupported_platform_bundle_schema() {
        let root = temp_dir();
        let bundle_manifest_path = root.join("platform-bundle.json");
        fs::write(
            &bundle_manifest_path,
            format!(
                "{{\"schema\":{},\"runtime_manifest\":{{\"schema\":1,\"release_id\":\"r1\",\"generated_at\":\"2026-02-01T00:00:00Z\",\"default_locale\":\"en\",\"supported_locales\":[\"en\"],\"id_map_hash\":\"sha256:{}\",\"mf2_packs\":{{}},\"icu_packs\":null,\"micro_locales\":null,\"budgets\":null,\"signing\":null}},\"id_map_path\":\"id-map.json\"}}",
                PLATFORM_BUNDLE_SCHEMA + 1,
                "00".repeat(32)
            ),
        )
        .expect("write bundle");

        let err = load_platform_bundle_manifest(&bundle_manifest_path).expect_err("schema error");
        assert!(matches!(err, PlatformBundleError::UnsupportedSchema(_)));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_malformed_platform_bundle_json() {
        let root = temp_dir();
        let bundle_manifest_path = root.join("platform-bundle.json");
        fs::write(&bundle_manifest_path, "{").expect("write malformed bundle");

        let err = load_platform_bundle_manifest(&bundle_manifest_path).expect_err("json error");
        assert!(matches!(err, PlatformBundleError::Json(_)));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn rejects_parent_traversal_in_id_map_path() {
        let root = temp_dir();
        let bundle_manifest_path = root.join("platform-bundle.json");
        let outside_id_map_path = root.join("..").join("outside-id-map.json");
        fs::write(&outside_id_map_path, br#"{"home.title":0}"#).expect("outside id map");

        let (valid_bundle_manifest_path, _) = write_valid_bundle_fixture(&root);
        let mut bundle_manifest =
            load_platform_bundle_manifest(&valid_bundle_manifest_path).expect("manifest");
        bundle_manifest.id_map_path = "../outside-id-map.json".to_string();
        write_platform_bundle_manifest(&bundle_manifest_path, &bundle_manifest).expect("manifest");

        let err = PlatformBundle::load(&bundle_manifest_path).expect_err("parent traversal");
        assert!(matches!(err, PlatformBundleError::ParentTraversal(_)));

        fs::remove_dir_all(&root).ok();
        fs::remove_file(&outside_id_map_path).ok();
    }

    #[test]
    fn rejects_absolute_id_map_path() {
        let root = temp_dir();
        let outside_id_map_path = std::env::temp_dir().join(format!(
            "mfs_i18n_platform_abs_id_map_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::write(&outside_id_map_path, br#"{"home.title":0}"#).expect("outside id map");

        let (valid_bundle_manifest_path, _) = write_valid_bundle_fixture(&root);
        let mut bundle_manifest =
            load_platform_bundle_manifest(&valid_bundle_manifest_path).expect("manifest");
        bundle_manifest.id_map_path = outside_id_map_path.to_string_lossy().into_owned();
        write_platform_bundle_manifest(&valid_bundle_manifest_path, &bundle_manifest)
            .expect("manifest");

        let err = PlatformBundle::load(&valid_bundle_manifest_path).expect_err("absolute path");
        assert!(matches!(
            err,
            PlatformBundleError::UnsupportedAbsolutePath(_)
        ));

        fs::remove_dir_all(&root).ok();
        fs::remove_file(&outside_id_map_path).ok();
    }

    #[test]
    fn rejects_parent_traversal_in_pack_path() {
        let root = temp_dir();
        let outside_pack_path = root.join("..").join("outside-pack.mf2pack");

        let (valid_bundle_manifest_path, pack_bytes) = write_valid_bundle_fixture(&root);
        fs::write(&outside_pack_path, &pack_bytes).expect("outside pack");

        let mut bundle_manifest =
            load_platform_bundle_manifest(&valid_bundle_manifest_path).expect("manifest");
        bundle_manifest
            .runtime_manifest
            .mf2_packs
            .get_mut("en")
            .expect("pack entry")
            .url = "../outside-pack.mf2pack".to_string();
        write_platform_bundle_manifest(&valid_bundle_manifest_path, &bundle_manifest)
            .expect("manifest");

        let err = PlatformBundle::load(&valid_bundle_manifest_path).expect_err("parent traversal");
        assert!(matches!(err, PlatformBundleError::ParentTraversal(_)));

        fs::remove_dir_all(&root).ok();
        fs::remove_file(&outside_pack_path).ok();
    }

    #[test]
    fn rejects_absolute_pack_path() {
        let root = temp_dir();
        let outside_pack_path = std::env::temp_dir().join(format!(
            "mfs_i18n_platform_abs_pack_{}.mf2pack",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        let (valid_bundle_manifest_path, pack_bytes) = write_valid_bundle_fixture(&root);
        fs::write(&outside_pack_path, &pack_bytes).expect("outside pack");

        let mut bundle_manifest =
            load_platform_bundle_manifest(&valid_bundle_manifest_path).expect("manifest");
        bundle_manifest
            .runtime_manifest
            .mf2_packs
            .get_mut("en")
            .expect("pack entry")
            .url = outside_pack_path.to_string_lossy().into_owned();
        write_platform_bundle_manifest(&valid_bundle_manifest_path, &bundle_manifest)
            .expect("manifest");

        let err = PlatformBundle::load(&valid_bundle_manifest_path).expect_err("absolute path");
        assert!(matches!(
            err,
            PlatformBundleError::UnsupportedAbsolutePath(_)
        ));

        fs::remove_dir_all(&root).ok();
        fs::remove_file(&outside_pack_path).ok();
    }
}

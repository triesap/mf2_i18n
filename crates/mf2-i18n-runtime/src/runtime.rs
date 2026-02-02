use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mf2_i18n_core::{
    execute, negotiate_lookup, Args, CatalogChain, FormatBackend, LanguageTag, PackCatalog,
    PluralCategory,
};

use crate::error::{RuntimeError, RuntimeResult};
use crate::id_map::IdMap;
use crate::loader::{load_id_map, load_manifest, parse_sha256};
use crate::manifest::{Manifest, PackEntry};

pub struct Runtime {
    manifest: Manifest,
    id_map: IdMap,
    packs: BTreeMap<String, PackCatalog>,
    parents: BTreeMap<String, String>,
    default_locale: LanguageTag,
    supported: Vec<LanguageTag>,
}

pub struct BasicFormatBackend;

impl FormatBackend for BasicFormatBackend {
    fn plural_category(&self, _value: f64) -> mf2_i18n_core::CoreResult<PluralCategory> {
        Ok(PluralCategory::Other)
    }

    fn format_number(
        &self,
        value: f64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_date(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_time(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_datetime(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_unit(
        &self,
        value: f64,
        unit_id: u32,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(format!("{value}:{unit_id}"))
    }

    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        let code = core::str::from_utf8(&code).unwrap_or("???");
        Ok(format!("{value}:{code}"))
    }
}

impl Runtime {
    pub fn load_from_paths(manifest_path: &Path, id_map_path: &Path) -> RuntimeResult<Self> {
        let manifest = load_manifest(manifest_path)?;
        let id_map = load_id_map(id_map_path)?;
        let expected_hash = parse_sha256(&manifest.id_map_hash)?;
        let actual_hash = id_map.hash()?;
        if expected_hash != actual_hash {
            return Err(RuntimeError::InvalidIdMap);
        }

        let pack_root = manifest_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut packs = BTreeMap::new();
        for (locale, entry) in &manifest.mf2_packs {
            let pack = load_pack(&pack_root, locale, entry, &expected_hash)?;
            packs.insert(locale.clone(), pack);
        }

        let mut parents = BTreeMap::new();
        if let Some(micro) = &manifest.micro_locales {
            for (child, parent) in micro {
                parents.insert(child.clone(), parent.clone());
            }
        }
        for (locale, entry) in &manifest.mf2_packs {
            if entry.kind == "overlay" {
                if let Some(parent) = &entry.parent {
                    parents.insert(locale.clone(), parent.clone());
                }
            }
        }

        let default_locale = LanguageTag::parse(&manifest.default_locale)?;
        let mut supported = Vec::new();
        for locale in &manifest.supported_locales {
            supported.push(LanguageTag::parse(locale)?);
        }

        Ok(Self {
            manifest,
            id_map,
            packs,
            parents,
            default_locale,
            supported,
        })
    }

    pub fn format(&self, locale: &str, key: &str, args: &Args) -> RuntimeResult<String> {
        let backend = BasicFormatBackend;
        self.format_with_backend(locale, key, args, &backend)
    }

    pub fn format_with_backend(
        &self,
        locale: &str,
        key: &str,
        args: &Args,
        backend: &dyn FormatBackend,
    ) -> RuntimeResult<String> {
        let locale_tag = LanguageTag::parse(locale)?;
        let negotiation = negotiate_lookup(&[locale_tag], &self.supported, &self.default_locale);
        let selected = negotiation.selected.normalized().to_string();
        let catalog_chain = self.catalog_chain_for(&selected)?;

        let message_id = self
            .id_map
            .get(key)
            .ok_or_else(|| RuntimeError::MissingMessage(key.to_string()))?;
        let program = catalog_chain
            .lookup(message_id)
            .ok_or_else(|| RuntimeError::MissingMessage(key.to_string()))?;
        let output = execute(program, args, backend)?;
        Ok(output)
    }

    fn catalog_chain_for(&self, locale: &str) -> RuntimeResult<CatalogChain<'_>> {
        let mut catalogs = Vec::new();
        let mut current = Some(locale.to_string());
        while let Some(tag) = current {
            if let Some(pack) = self.packs.get(&tag) {
                catalogs.push(pack as &dyn mf2_i18n_core::Catalog);
            }
            current = self.parents.get(&tag).cloned();
        }
        if catalogs.is_empty() {
            return Err(RuntimeError::MissingLocale(locale.to_string()));
        }
        Ok(CatalogChain::new(catalogs))
    }
}

fn load_pack(
    root: &Path,
    locale: &str,
    entry: &PackEntry,
    id_map_hash: &[u8; 32],
) -> RuntimeResult<PackCatalog> {
    let pack_path = root.join(&entry.url);
    let bytes = fs::read(&pack_path)?;
    if bytes.len() as u64 != entry.size {
        return Err(RuntimeError::HashMismatch(locale.to_string()));
    }
    let expected_hash = parse_sha256(&entry.hash)?;
    let actual_hash = sha256(&bytes);
    if expected_hash != actual_hash {
        return Err(RuntimeError::HashMismatch(locale.to_string()));
    }
    Ok(PackCatalog::decode(&bytes, id_map_hash)?)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::Runtime;
    use crate::id_map::IdMap;
    use crate::manifest::{Manifest, PackEntry};
    use mf2_i18n_core::{Args, PackKind};
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        path.push(format!("mf2_i18n_runtime_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn build_pack_bytes(id_map_hash: [u8; 32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MF2PACK\0");
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.push(match PackKind::Base {
            PackKind::Base => 0,
            PackKind::Overlay => 1,
            PackKind::IcuData => 2,
        });
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&id_map_hash);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());

        let mut string_pool = Vec::new();
        string_pool.extend_from_slice(&2u32.to_le_bytes());
        string_pool.extend_from_slice(&2u32.to_le_bytes());
        string_pool.extend_from_slice(b"hi");
        string_pool.extend_from_slice(&4u32.to_le_bytes());
        string_pool.extend_from_slice(b"name");

        let mut message_meta = Vec::new();
        message_meta.extend_from_slice(&1u32.to_le_bytes());
        message_meta.extend_from_slice(&0u32.to_le_bytes());
        message_meta.extend_from_slice(&0u32.to_le_bytes());

        let mut case_tables = Vec::new();
        case_tables.extend_from_slice(&0u32.to_le_bytes());

        let mut message_index = Vec::new();
        message_index.extend_from_slice(&1u32.to_le_bytes());
        message_index.extend_from_slice(&0u32.to_le_bytes());

        let mut message = Vec::new();
        message.extend_from_slice(&0u32.to_le_bytes());
        message.extend_from_slice(&2u32.to_le_bytes());
        message.push(0);
        message.extend_from_slice(&0u32.to_le_bytes());
        message.push(11);
        let mut bytecode_blob = Vec::new();
        bytecode_blob.extend_from_slice(&(message.len() as u32).to_le_bytes());
        bytecode_blob.extend_from_slice(&message);

        let section_count = 5u16;
        bytes.extend_from_slice(&section_count.to_le_bytes());
        let dir_start = bytes.len();
        let dir_len = section_count as usize * (1 + 4 + 4);
        bytes.resize(dir_start + dir_len, 0);
        let mut offset = bytes.len() as u32;

        let sections = vec![
            (1u8, string_pool),
            (2u8, message_index),
            (3u8, bytecode_blob),
            (4u8, case_tables),
            (5u8, message_meta),
        ];

        for (idx, (section_type, data)) in sections.into_iter().enumerate() {
            let entry_offset = dir_start + idx * 9;
            bytes[entry_offset] = section_type;
            bytes[entry_offset + 1..entry_offset + 5].copy_from_slice(&offset.to_le_bytes());
            bytes[entry_offset + 5..entry_offset + 9]
                .copy_from_slice(&(data.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&data);
            offset += data.len() as u32;
        }

        bytes
    }

    #[test]
    fn runtime_formats_message() {
        let root = temp_dir();
        let packs_dir = root.join("packs");
        fs::create_dir_all(&packs_dir).expect("packs");

        let id_map_json = r#"{"home.title": 0}"#;
        let id_map = IdMap::from_json(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash);
        let pack_path = packs_dir.join("en.mf2pack");
        fs::write(&pack_path, &pack_bytes).expect("write pack");

        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: format!("sha256:{}", hex::encode(super::sha256(&pack_bytes))),
                size: pack_bytes.len() as u64,
                content_encoding: "identity".to_string(),
                pack_schema: 0,
                parent: None,
            },
        );

        let manifest = Manifest {
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
        };

        let manifest_path = root.join("manifest.json");
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).expect("json"))
            .expect("write manifest");

        let id_map_path = root.join("id_map.json");
        fs::write(&id_map_path, id_map_json).expect("write id map");

        let runtime = Runtime::load_from_paths(&manifest_path, &id_map_path).expect("runtime");
        let args = Args::new();
        let output = runtime.format("en", "home.title", &args).expect("format");
        assert_eq!(output, "hi");

        fs::remove_dir_all(&root).ok();
    }
}

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use mf2_i18n_core::{Args, MessageId};
use mf2_i18n_embedded::{EmbeddedPack, EmbeddedRuntime};
use mf2_i18n_runtime::{IdMap, Runtime, parse_sha256_literal};

use crate::error::{NativeError, NativeResult};

pub enum NativeRuntime {
    Embedded(EmbeddedRuntime),
    Filesystem(Runtime),
}

pub struct NativeLocalizer {
    runtime: Option<NativeRuntime>,
    default_locale: String,
    active_locale: RwLock<String>,
}

impl NativeLocalizer {
    pub fn fallback(default_locale: &str) -> Self {
        Self {
            runtime: None,
            default_locale: default_locale.to_owned(),
            active_locale: RwLock::new(default_locale.to_owned()),
        }
    }

    pub fn from_embedded_artifacts(
        default_locale: &str,
        id_map_json: &[u8],
        id_map_hash: &[u8],
        packs: &[EmbeddedPack<'_>],
    ) -> NativeResult<Self> {
        let id_map = IdMap::from_bytes(id_map_json)?;
        let id_map_hash_text = std::str::from_utf8(id_map_hash)?;
        let parsed_hash = parse_sha256_literal(id_map_hash_text.trim())?;
        let runtime = EmbeddedRuntime::new(
            to_embedded_id_map(&id_map),
            parsed_hash,
            packs,
            default_locale,
        )?;
        Ok(Self {
            runtime: Some(NativeRuntime::Embedded(runtime)),
            default_locale: default_locale.to_owned(),
            active_locale: RwLock::new(default_locale.to_owned()),
        })
    }

    pub fn from_paths(manifest_path: &Path, id_map_path: &Path) -> NativeResult<Self> {
        let runtime = Runtime::load_from_paths(manifest_path, id_map_path)?;
        let default_locale = runtime.default_locale().to_owned();
        Ok(Self {
            runtime: Some(NativeRuntime::Filesystem(runtime)),
            active_locale: RwLock::new(default_locale.clone()),
            default_locale,
        })
    }

    pub fn is_ready(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn set_locale(&self, locale: &str) {
        if locale.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.active_locale.write() {
            *guard = locale.to_owned();
        }
    }

    pub fn locale(&self) -> String {
        match self.active_locale.read() {
            Ok(guard) => guard.clone(),
            Err(_) => self.default_locale.clone(),
        }
    }

    pub fn tr(&self, key: &str) -> String {
        let args = Args::new();
        self.tr_with_args(key, &args)
    }

    pub fn tr_with_args(&self, key: &str, args: &Args) -> String {
        self.format(key, args).unwrap_or_else(|_| key.to_owned())
    }

    pub fn format(&self, key: &str, args: &Args) -> NativeResult<String> {
        let locale = self.locale();
        match self.runtime.as_ref() {
            Some(NativeRuntime::Embedded(runtime)) => Ok(runtime.format(&locale, key, args)?),
            Some(NativeRuntime::Filesystem(runtime)) => Ok(runtime.format(&locale, key, args)?),
            None => Err(NativeError::NotInitialized),
        }
    }
}

fn to_embedded_id_map(id_map: &IdMap) -> BTreeMap<String, MessageId> {
    let mut out = BTreeMap::new();
    for (key, id) in id_map.entries() {
        out.insert(key.to_owned(), id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::NativeLocalizer;
    use mf2_i18n_core::{Args, PackKind};
    use mf2_i18n_embedded::EmbeddedPack;
    use mf2_i18n_runtime::IdMap;
    use sha2::{Digest, Sha256};
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
        path.push(format!("mf2_i18n_native_{nanos}"));
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

    fn sha256(bytes: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.finalize().into()
    }

    #[test]
    fn formats_from_embedded_artifacts() {
        let id_map_json = br#"{"home.title": 0}"#;
        let id_map = IdMap::from_bytes(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash);
        let hash_literal = format!("sha256:{}", hex::encode(id_map_hash));
        let packs = [EmbeddedPack {
            locale: "en",
            bytes: &pack_bytes,
        }];
        let localizer = NativeLocalizer::from_embedded_artifacts(
            "en",
            id_map_json,
            hash_literal.as_bytes(),
            &packs,
        )
        .expect("localizer");

        assert!(localizer.is_ready());
        assert_eq!(localizer.tr("home.title"), "hi");
    }

    #[test]
    fn fallback_localizer_uses_key() {
        let localizer = NativeLocalizer::fallback("en");
        assert!(!localizer.is_ready());
        assert_eq!(localizer.tr("home.title"), "home.title");
    }

    #[test]
    fn formats_from_filesystem_artifacts() {
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
            mf2_i18n_runtime::PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: format!("sha256:{}", hex::encode(sha256(&pack_bytes))),
                size: pack_bytes.len() as u64,
                content_encoding: "identity".to_string(),
                pack_schema: 0,
                parent: None,
            },
        );

        let manifest = mf2_i18n_runtime::Manifest {
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
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("json"),
        )
        .expect("write manifest");

        let id_map_path = root.join("id_map.json");
        fs::write(&id_map_path, id_map_json).expect("write id map");

        let localizer =
            NativeLocalizer::from_paths(&manifest_path, &id_map_path).expect("localizer");
        let args = Args::new();
        let output = localizer.format("home.title", &args).expect("format");
        assert_eq!(output, "hi");

        fs::remove_dir_all(&root).ok();
    }
}

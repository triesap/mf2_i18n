use std::collections::BTreeMap;
use std::path::Path;
use std::sync::RwLock;

use mfs_i18n_core::{Args, FormatBackend, LanguageTag, MessageId, negotiate_lookup};
use mfs_i18n_embedded::{EmbeddedPack, EmbeddedRuntime};
use mfs_i18n_runtime::{IdMap, Runtime, parse_sha256_literal};
use mfs_i18n_std::StdFormatBackend;

use crate::error::{NativeError, NativeResult};

pub enum NativeRuntime {
    Embedded(EmbeddedRuntime),
    Filesystem(Runtime),
}

pub struct NativeLocalizer {
    runtime: Option<NativeRuntime>,
    default_locale: String,
    supported_locales: Vec<LanguageTag>,
    preferred_locales: RwLock<Vec<String>>,
    active_locale: RwLock<String>,
}

impl NativeLocalizer {
    pub fn fallback(default_locale: &str) -> Self {
        Self {
            runtime: None,
            default_locale: default_locale.to_owned(),
            supported_locales: Vec::new(),
            preferred_locales: RwLock::new(vec![default_locale.to_owned()]),
            active_locale: RwLock::new(default_locale.to_owned()),
        }
    }

    pub fn from_embedded_artifacts_or_fallback(
        default_locale: &str,
        id_map_json: &[u8],
        id_map_hash: &[u8],
        packs: &[EmbeddedPack<'_>],
    ) -> Self {
        Self::from_embedded_artifacts(default_locale, id_map_json, id_map_hash, packs)
            .unwrap_or_else(|_| Self::fallback(default_locale))
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
        let default_locale = runtime.default_locale().to_owned();
        let supported_locales = runtime.supported_locales().to_vec();
        Ok(Self {
            runtime: Some(NativeRuntime::Embedded(runtime)),
            default_locale: default_locale.clone(),
            supported_locales,
            preferred_locales: RwLock::new(vec![default_locale.clone()]),
            active_locale: RwLock::new(default_locale),
        })
    }

    pub fn from_paths_or_fallback(
        fallback_locale: &str,
        manifest_path: &Path,
        id_map_path: &Path,
    ) -> Self {
        Self::from_paths(manifest_path, id_map_path)
            .unwrap_or_else(|_| Self::fallback(fallback_locale))
    }

    pub fn from_paths(manifest_path: &Path, id_map_path: &Path) -> NativeResult<Self> {
        let runtime = Runtime::load_from_paths(manifest_path, id_map_path)?;
        let default_locale = runtime.default_locale().to_owned();
        let supported_locales = runtime.supported_locales().to_vec();
        Ok(Self {
            runtime: Some(NativeRuntime::Filesystem(runtime)),
            supported_locales,
            preferred_locales: RwLock::new(vec![default_locale.clone()]),
            active_locale: RwLock::new(default_locale.clone()),
            default_locale,
        })
    }

    pub fn is_ready(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn set_preferred_locales<I, S>(&self, preferred_locales: I) -> NativeResult<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let requested = normalize_requested_locales(preferred_locales, &self.default_locale);
        let parsed = parse_requested_locales(&requested)?;
        let selected = self.select_locale(&parsed);

        if let Ok(mut guard) = self.preferred_locales.write() {
            *guard = requested;
        }
        if let Ok(mut guard) = self.active_locale.write() {
            *guard = selected.clone();
        }

        Ok(selected)
    }

    pub fn set_locale(&self, locale: &str) -> NativeResult<String> {
        self.set_preferred_locales([locale])
    }

    pub fn locale(&self) -> String {
        match self.active_locale.read() {
            Ok(guard) => guard.clone(),
            Err(_) => self.default_locale.clone(),
        }
    }

    pub fn default_locale(&self) -> String {
        self.default_locale.clone()
    }

    pub fn preferred_locales(&self) -> Vec<String> {
        match self.preferred_locales.read() {
            Ok(guard) => guard.clone(),
            Err(_) => vec![self.default_locale.clone()],
        }
    }

    pub fn supported_locales(&self) -> Vec<String> {
        if self.supported_locales.is_empty() {
            return vec![self.default_locale.clone()];
        }
        self.supported_locales
            .iter()
            .map(|tag| tag.normalized().to_owned())
            .collect()
    }

    pub fn tr(&self, key: &str) -> NativeResult<String> {
        let args = Args::new();
        self.tr_with_args(key, &args)
    }

    pub fn tr_with_args(&self, key: &str, args: &Args) -> NativeResult<String> {
        self.format(key, args)
    }

    pub fn tr_or_key(&self, key: &str) -> String {
        let args = Args::new();
        self.tr_with_args_or_key(key, &args)
    }

    pub fn tr_with_args_or_key(&self, key: &str, args: &Args) -> String {
        self.tr_with_args(key, args)
            .unwrap_or_else(|_| key.to_owned())
    }

    pub fn format(&self, key: &str, args: &Args) -> NativeResult<String> {
        let locale = self.locale();
        let backend =
            StdFormatBackend::new(&locale).map_err(|err| NativeError::Core(err.to_string()))?;
        self.format_with_backend(key, args, &backend)
    }

    pub fn format_with_backend(
        &self,
        key: &str,
        args: &Args,
        backend: &dyn FormatBackend,
    ) -> NativeResult<String> {
        let locale = self.locale();
        match self.runtime.as_ref() {
            Some(NativeRuntime::Embedded(runtime)) => {
                Ok(runtime.format_with_backend(&locale, key, args, backend)?)
            }
            Some(NativeRuntime::Filesystem(runtime)) => {
                Ok(runtime.format_with_backend(&locale, key, args, backend)?)
            }
            None => Err(NativeError::NotInitialized),
        }
    }

    fn select_locale(&self, requested: &[LanguageTag]) -> String {
        if self.supported_locales.is_empty() {
            return requested
                .first()
                .map(|tag| tag.normalized().to_owned())
                .unwrap_or_else(|| self.default_locale.clone());
        }

        let default_locale = LanguageTag::parse(&self.default_locale)
            .ok()
            .unwrap_or_else(|| self.supported_locales[0].clone());
        negotiate_lookup(requested, &self.supported_locales, &default_locale)
            .selected
            .normalized()
            .to_owned()
    }
}

fn to_embedded_id_map(id_map: &IdMap) -> BTreeMap<String, MessageId> {
    let mut out = BTreeMap::new();
    for (key, id) in id_map.entries() {
        out.insert(key.to_owned(), id);
    }
    out
}

fn normalize_requested_locales<I, S>(preferred_locales: I, default_locale: &str) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut requested = preferred_locales
        .into_iter()
        .map(|locale| locale.as_ref().trim().to_owned())
        .filter(|locale| !locale.is_empty())
        .collect::<Vec<_>>();
    if requested.is_empty() {
        requested.push(default_locale.to_owned());
    }
    requested
}

fn parse_requested_locales(locales: &[String]) -> NativeResult<Vec<LanguageTag>> {
    locales
        .iter()
        .map(|locale| LanguageTag::parse(locale).map_err(NativeError::from))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::NativeLocalizer;
    use mfs_i18n_build::compiler::compile_message;
    use mfs_i18n_build::pack_encode::{PackBuildInput, encode_pack};
    use mfs_i18n_build::parser::parse_message;
    use mfs_i18n_core::{Args, MessageId, PackKind};
    use mfs_i18n_embedded::EmbeddedPack;
    use mfs_i18n_runtime::IdMap;
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
        path.push(format!("mfs_i18n_native_{nanos}"));
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
        let pack_bytes = build_pack_bytes(id_map_hash, "en", "hi");
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
        assert_eq!(localizer.tr("home.title").expect("translation"), "hi");
        assert_eq!(localizer.tr_or_key("home.title"), "hi");
        assert_eq!(localizer.default_locale(), "en");
        assert_eq!(localizer.supported_locales(), vec!["en".to_string()]);
    }

    #[test]
    fn fallback_localizer_uses_key() {
        let localizer = NativeLocalizer::fallback("en");
        assert!(!localizer.is_ready());
        assert!(matches!(
            localizer.tr("home.title"),
            Err(crate::NativeError::NotInitialized)
        ));
        assert_eq!(localizer.tr_or_key("home.title"), "home.title");
        assert_eq!(localizer.preferred_locales(), vec!["en".to_string()]);
    }

    #[test]
    fn formats_from_filesystem_artifacts() {
        let root = temp_dir();
        let packs_dir = root.join("packs");
        fs::create_dir_all(&packs_dir).expect("packs");

        let id_map_json = r#"{"home.title": 0}"#;
        let id_map = IdMap::from_json(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash, "en", "hi");
        let pack_path = packs_dir.join("en.mf2pack");
        fs::write(&pack_path, &pack_bytes).expect("write pack");

        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            mfs_i18n_runtime::PackEntry {
                kind: "base".to_string(),
                url: "packs/en.mf2pack".to_string(),
                hash: format!("sha256:{}", hex::encode(sha256(&pack_bytes))),
                size: pack_bytes.len() as u64,
                content_encoding: "identity".to_string(),
                pack_schema: 0,
                parent: None,
            },
        );

        let manifest = mfs_i18n_runtime::Manifest {
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

    #[test]
    fn negotiates_preferred_locales_against_supported_locales() {
        let id_map_json = br#"{"home.title": 0}"#;
        let id_map = IdMap::from_bytes(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let en_pack = build_pack_bytes(id_map_hash, "en", "hi");
        let fr_pack = build_pack_bytes(id_map_hash, "fr", "salut");
        let hash_literal = format!("sha256:{}", hex::encode(id_map_hash));
        let packs = [
            EmbeddedPack {
                locale: "en",
                bytes: &en_pack,
            },
            EmbeddedPack {
                locale: "fr",
                bytes: &fr_pack,
            },
        ];
        let localizer = NativeLocalizer::from_embedded_artifacts(
            "en",
            id_map_json,
            hash_literal.as_bytes(),
            &packs,
        )
        .expect("localizer");

        let selected = localizer
            .set_preferred_locales(["fr-CA", "en-GB"])
            .expect("locale");
        assert_eq!(selected, "fr");
        assert_eq!(localizer.locale(), "fr");
        assert_eq!(
            localizer.preferred_locales(),
            vec!["fr-CA".to_string(), "en-GB".to_string()]
        );
        assert_eq!(
            localizer.supported_locales(),
            vec!["en".to_string(), "fr".to_string()]
        );
        assert_eq!(localizer.tr("home.title").expect("translation"), "salut");
    }

    #[test]
    fn formats_numbers_with_default_backend() {
        let id_map_json = br#"{"home.total": 0}"#;
        let id_map = IdMap::from_bytes(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let en_pack = build_pack_bytes(id_map_hash, "en", "{ $count:number }");
        let fr_pack = build_pack_bytes(id_map_hash, "fr", "{ $count:number }");
        let hash_literal = format!("sha256:{}", hex::encode(id_map_hash));
        let packs = [
            EmbeddedPack {
                locale: "en",
                bytes: &en_pack,
            },
            EmbeddedPack {
                locale: "fr",
                bytes: &fr_pack,
            },
        ];
        let localizer = NativeLocalizer::from_embedded_artifacts(
            "en",
            id_map_json,
            hash_literal.as_bytes(),
            &packs,
        )
        .expect("localizer");
        localizer.set_preferred_locales(["fr-BE"]).expect("locale");

        let mut args = Args::new();
        args.insert("count", mfs_i18n_core::Value::Num(12345.5));
        assert_eq!(
            localizer.format("home.total", &args).expect("format"),
            "12\u{202f}345,5"
        );
    }

    #[test]
    fn fallback_localizer_tracks_requested_locale_without_runtime() {
        let localizer = NativeLocalizer::fallback("en");
        let selected = localizer
            .set_preferred_locales(["fr-CA", "en-GB"])
            .expect("locale");
        assert_eq!(selected, "fr-CA");
        assert_eq!(localizer.locale(), "fr-CA");
    }

    #[test]
    fn fallback_helpers_do_not_hide_strict_translation_failures() {
        let id_map_json = br#"{"home.distance": 0}"#;
        let id_map = IdMap::from_bytes(id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack = build_pack_bytes(id_map_hash, "en", "{ $distance:unit }");
        let hash_literal = format!("sha256:{}", hex::encode(id_map_hash));
        let packs = [EmbeddedPack {
            locale: "en",
            bytes: &pack,
        }];
        let localizer = NativeLocalizer::from_embedded_artifacts(
            "en",
            id_map_json,
            hash_literal.as_bytes(),
            &packs,
        )
        .expect("localizer");

        let mut args = Args::new();
        args.insert(
            "distance",
            mfs_i18n_core::Value::Unit {
                value: 12.5,
                unit_id: 7,
            },
        );

        let err = localizer
            .tr_with_args("home.distance", &args)
            .expect_err("strict translation should fail");
        assert_eq!(
            err.to_string(),
            "core error: unsupported: unit formatting requires unit label data"
        );
        assert_eq!(
            localizer.tr_with_args_or_key("home.distance", &args),
            "home.distance"
        );
    }
}

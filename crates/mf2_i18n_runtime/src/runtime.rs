use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mf2_i18n_core::{
    Args, CatalogChain, DateTimeValue, FormatBackend, LanguageTag, PackCatalog, PackKind,
    PluralCategory, decode_string_pool, execute, negotiate_lookup, parse_pack_header,
    parse_section_directory,
};
use mf2_i18n_std::StdFormatBackend;

use crate::error::{RuntimeError, RuntimeResult};
use crate::id_map::IdMap;
use crate::loader::{load_manifest, parse_sha256};
use crate::manifest::{Manifest, PackEntry};

const MANIFEST_SCHEMA: u32 = 1;
const PACK_SCHEMA: u32 = 0;
const SECTION_STRING_POOL: u8 = 1;

pub struct Runtime {
    id_map: IdMap,
    packs: BTreeMap<String, PackCatalog>,
    parents: BTreeMap<String, String>,
    default_locale: LanguageTag,
    supported: Vec<LanguageTag>,
}

#[derive(Debug, Clone)]
pub struct RuntimeParts {
    pub manifest: Manifest,
    pub id_map_json: Vec<u8>,
    pub packs: BTreeMap<String, Vec<u8>>,
}

impl RuntimeParts {
    pub fn new(
        manifest: Manifest,
        id_map_json: impl Into<Vec<u8>>,
        packs: BTreeMap<String, Vec<u8>>,
    ) -> Self {
        Self {
            manifest,
            id_map_json: id_map_json.into(),
            packs,
        }
    }
}

pub struct BasicFormatBackend;
pub struct UnsupportedFormatBackend;

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
        value: DateTimeValue,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_time(
        &self,
        value: DateTimeValue,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_datetime(
        &self,
        value: DateTimeValue,
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

impl FormatBackend for UnsupportedFormatBackend {
    fn plural_category(&self, _value: f64) -> mf2_i18n_core::CoreResult<PluralCategory> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "plural selection requires a format backend",
        ))
    }

    fn format_number(
        &self,
        _value: f64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "number formatting requires a format backend",
        ))
    }

    fn format_date(
        &self,
        _value: DateTimeValue,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "date formatting requires a format backend",
        ))
    }

    fn format_time(
        &self,
        _value: DateTimeValue,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "time formatting requires a format backend",
        ))
    }

    fn format_datetime(
        &self,
        _value: DateTimeValue,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "datetime formatting requires a format backend",
        ))
    }

    fn format_unit(
        &self,
        _value: f64,
        _unit_id: u32,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "unit formatting requires a format backend",
        ))
    }

    fn format_currency(
        &self,
        _value: f64,
        _code: [u8; 3],
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> mf2_i18n_core::CoreResult<String> {
        Err(mf2_i18n_core::CoreError::Unsupported(
            "currency formatting requires a format backend",
        ))
    }
}

impl Runtime {
    pub fn load_from_paths(manifest_path: &Path, id_map_path: &Path) -> RuntimeResult<Self> {
        let manifest = load_manifest(manifest_path)?;
        let id_map_json = fs::read(id_map_path)?;
        let pack_root = manifest_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut packs = BTreeMap::new();
        for (locale, entry) in &manifest.mf2_packs {
            packs.insert(locale.clone(), fs::read(pack_root.join(&entry.url))?);
        }

        Self::from_parts(RuntimeParts::new(manifest, id_map_json, packs))
    }

    pub fn from_parts(parts: RuntimeParts) -> RuntimeResult<Self> {
        validate_manifest(&parts.manifest, &parts.packs)?;
        let id_map = IdMap::from_bytes(&parts.id_map_json)?;
        let expected_hash = parse_sha256(&parts.manifest.id_map_hash)?;
        let actual_hash = id_map.hash()?;
        if expected_hash != actual_hash {
            return Err(RuntimeError::InvalidIdMap);
        }

        let parents = parent_map(&parts.manifest)?;
        let mut packs = BTreeMap::new();
        for locale in &parts.manifest.supported_locales {
            let normalized_locale = normalize_locale(locale)?;
            let entry = parts
                .manifest
                .mf2_packs
                .get(locale)
                .ok_or_else(|| RuntimeError::MissingPack(locale.clone()))?;
            let bytes = parts
                .packs
                .get(locale)
                .ok_or_else(|| RuntimeError::MissingPack(locale.clone()))?;
            let pack = load_pack_bytes(locale, entry, &expected_hash, bytes)?;
            packs.insert(normalized_locale, pack);
        }

        let default_locale = LanguageTag::parse(&parts.manifest.default_locale)?;
        let mut supported = Vec::new();
        for locale in &parts.manifest.supported_locales {
            supported.push(LanguageTag::parse(locale)?);
        }

        Ok(Self {
            id_map,
            packs,
            parents,
            default_locale,
            supported,
        })
    }

    pub fn format(&self, locale: &str, key: &str, args: &Args) -> RuntimeResult<String> {
        let backend =
            StdFormatBackend::new(locale).map_err(|err| RuntimeError::Core(err.to_string()))?;
        self.format_with_backend(locale, key, args, &backend)
    }

    pub fn default_locale(&self) -> &str {
        self.default_locale.normalized()
    }

    pub fn supported_locales(&self) -> &[LanguageTag] {
        &self.supported
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

fn validate_manifest(manifest: &Manifest, packs: &BTreeMap<String, Vec<u8>>) -> RuntimeResult<()> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(RuntimeError::InvalidManifest(format!(
            "unsupported manifest schema {}",
            manifest.schema
        )));
    }

    let mut supported = BTreeSet::new();
    for locale in &manifest.supported_locales {
        let normalized_locale = normalize_locale(locale)?;
        if !supported.insert(normalized_locale) {
            return Err(RuntimeError::InvalidManifest(format!(
                "duplicate supported locale {locale}"
            )));
        }
        if !manifest.mf2_packs.contains_key(locale) {
            return Err(RuntimeError::MissingPack(locale.clone()));
        }
        if !packs.contains_key(locale) {
            return Err(RuntimeError::MissingPack(locale.clone()));
        }
    }

    let default_locale = normalize_locale(&manifest.default_locale)?;
    if !supported.contains(&default_locale) {
        return Err(RuntimeError::InvalidManifest(format!(
            "default locale {} is not supported",
            manifest.default_locale
        )));
    }

    for locale in manifest.mf2_packs.keys() {
        let normalized_locale = normalize_locale(locale)?;
        if !supported.contains(&normalized_locale) {
            return Err(RuntimeError::InvalidManifest(format!(
                "pack locale {locale} is not supported"
            )));
        }
    }

    for locale in packs.keys() {
        if !manifest.mf2_packs.contains_key(locale) {
            return Err(RuntimeError::UnexpectedPack(locale.clone()));
        }
    }

    Ok(())
}

fn parent_map(manifest: &Manifest) -> RuntimeResult<BTreeMap<String, String>> {
    let mut parents = BTreeMap::new();
    if let Some(micro) = &manifest.micro_locales {
        for (child, parent) in micro {
            let child_locale = normalize_locale(child)?;
            let parent_locale = normalize_locale(parent)?;
            if !manifest.mf2_packs.contains_key(child) {
                return Err(RuntimeError::InvalidManifest(format!(
                    "micro-locale child {child} has no pack"
                )));
            }
            if !manifest.mf2_packs.contains_key(parent) {
                return Err(RuntimeError::InvalidManifest(format!(
                    "micro-locale parent {parent} has no pack"
                )));
            }
            parents.insert(child_locale, parent_locale);
        }
    }

    for (locale, entry) in &manifest.mf2_packs {
        let expected_kind = entry_kind(locale, &entry.kind)?;
        let locale_tag = normalize_locale(locale)?;
        match expected_kind {
            PackKind::Overlay => {
                let parent = entry.parent.as_ref().ok_or_else(|| {
                    RuntimeError::InvalidManifest(format!(
                        "overlay pack {locale} is missing parent"
                    ))
                })?;
                if !manifest.mf2_packs.contains_key(parent) {
                    return Err(RuntimeError::InvalidManifest(format!(
                        "overlay parent {parent} has no pack"
                    )));
                }
                let parent_tag = normalize_locale(parent)?;
                if let Some(existing) = parents.insert(locale_tag, parent_tag.clone()) {
                    if existing != parent_tag {
                        return Err(RuntimeError::InvalidManifest(format!(
                            "micro-locale parent for {locale} conflicts with pack parent"
                        )));
                    }
                }
            }
            PackKind::Base => {
                if entry.parent.is_some() {
                    return Err(RuntimeError::InvalidManifest(format!(
                        "base pack {locale} must not declare parent"
                    )));
                }
            }
            PackKind::IcuData => {}
        }
    }

    Ok(parents)
}

fn load_pack_bytes(
    locale: &str,
    entry: &PackEntry,
    id_map_hash: &[u8; 32],
    bytes: &[u8],
) -> RuntimeResult<PackCatalog> {
    if bytes.len() as u64 != entry.size {
        return Err(RuntimeError::PackSizeMismatch(locale.to_string()));
    }
    let expected_hash = parse_sha256(&entry.hash)?;
    let actual_hash = sha256(bytes);
    if expected_hash != actual_hash {
        return Err(RuntimeError::PackHashMismatch(locale.to_string()));
    }
    let metadata = pack_metadata(bytes)?;
    if u32::from(metadata.schema_version) != entry.pack_schema || entry.pack_schema != PACK_SCHEMA {
        return Err(RuntimeError::PackSchemaMismatch(locale.to_string()));
    }
    let expected_kind = entry_kind(locale, &entry.kind)?;
    if metadata.kind != expected_kind {
        return Err(RuntimeError::PackKindMismatch {
            locale: locale.to_string(),
            expected: pack_kind_literal(expected_kind).to_string(),
            actual: pack_kind_literal(metadata.kind).to_string(),
        });
    }
    let expected_locale = normalize_locale(locale)?;
    let actual_locale = normalize_locale(&metadata.locale)?;
    if actual_locale != expected_locale {
        return Err(RuntimeError::PackLocaleMismatch {
            expected: expected_locale,
            actual: actual_locale,
        });
    }
    let expected_parent = entry.parent.as_deref().map(normalize_locale).transpose()?;
    let actual_parent = metadata
        .parent
        .as_deref()
        .map(normalize_locale)
        .transpose()?;
    if actual_parent != expected_parent {
        return Err(RuntimeError::PackParentMismatch {
            locale: locale.to_string(),
            expected: expected_parent,
            actual: actual_parent,
        });
    }
    Ok(PackCatalog::decode(&bytes, id_map_hash)?)
}

struct PackMetadata {
    schema_version: u16,
    kind: PackKind,
    locale: String,
    parent: Option<String>,
}

fn pack_metadata(bytes: &[u8]) -> RuntimeResult<PackMetadata> {
    let (header, mut cursor) = parse_pack_header(bytes)?;
    let section_count = read_u16(bytes, &mut cursor)? as usize;
    let sections = parse_section_directory(bytes, cursor, section_count)?;
    let section = sections
        .iter()
        .find(|section| section.section_type == SECTION_STRING_POOL)
        .ok_or_else(|| RuntimeError::InvalidManifest("pack missing string pool".to_string()))?;
    let start = section.offset as usize;
    let end = start
        .checked_add(section.length as usize)
        .ok_or_else(|| RuntimeError::InvalidManifest("pack string pool overflow".to_string()))?;
    if end > bytes.len() {
        return Err(RuntimeError::InvalidManifest(
            "pack string pool out of bounds".to_string(),
        ));
    }
    let string_pool = decode_string_pool(&bytes[start..end])?;
    let locale = string_pool
        .get(header.locale_tag_sidx as usize)
        .cloned()
        .ok_or_else(|| RuntimeError::InvalidManifest("pack locale tag missing".to_string()))?;
    let parent = header
        .parent_tag_sidx
        .map(|idx| {
            string_pool
                .get(idx as usize)
                .cloned()
                .ok_or_else(|| RuntimeError::InvalidManifest("pack parent tag missing".to_string()))
        })
        .transpose()?;

    Ok(PackMetadata {
        schema_version: header.schema_version,
        kind: header.pack_kind,
        locale,
        parent,
    })
}

fn read_u16(bytes: &[u8], cursor: &mut usize) -> RuntimeResult<u16> {
    let end = *cursor + 2;
    if end > bytes.len() {
        return Err(RuntimeError::InvalidManifest(
            "pack section count missing".to_string(),
        ));
    }
    let value = u16::from_le_bytes([bytes[*cursor], bytes[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn entry_kind(locale: &str, kind: &str) -> RuntimeResult<PackKind> {
    match kind {
        "base" => Ok(PackKind::Base),
        "overlay" => Ok(PackKind::Overlay),
        "icu_data" => Ok(PackKind::IcuData),
        _ => Err(RuntimeError::InvalidManifest(format!(
            "unsupported pack kind {kind} for locale {locale}"
        ))),
    }
}

fn pack_kind_literal(kind: PackKind) -> &'static str {
    match kind {
        PackKind::Base => "base",
        PackKind::Overlay => "overlay",
        PackKind::IcuData => "icu_data",
    }
}

fn normalize_locale(locale: &str) -> RuntimeResult<String> {
    Ok(LanguageTag::parse(locale)?.normalized().to_string())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::{BasicFormatBackend, Runtime, RuntimeParts};
    use crate::RuntimeError;
    use crate::id_map::IdMap;
    use crate::manifest::{Manifest, PackEntry};
    use mf2_i18n_build::compiler::compile_message;
    use mf2_i18n_build::pack_encode::{PackBuildInput, encode_pack};
    use mf2_i18n_build::parser::parse_message;
    use mf2_i18n_core::{
        Args, DateTimeValue, FormatBackend, FormatterOption, FormatterOptionValue, MessageId,
        PackKind, PluralCategory, Value,
    };
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

    fn build_pack_bytes(
        id_map_hash: [u8; 32],
        locale_tag: &str,
        parent_tag: Option<&str>,
        pack_kind: PackKind,
        source: &str,
    ) -> Vec<u8> {
        let message = parse_message(source).expect("parse");
        let compiled = compile_message(&message).expect("compile");
        let mut messages = BTreeMap::new();
        messages.insert(MessageId::new(0), compiled.program);
        encode_pack(&PackBuildInput {
            pack_kind,
            id_map_hash,
            locale_tag: locale_tag.to_string(),
            parent_tag: parent_tag.map(str::to_string),
            build_epoch_ms: 0,
            messages,
        })
    }

    #[test]
    fn runtime_formats_message() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.title", "hi");
        assert_eq!(runtime.default_locale(), "en");
        let args = Args::new();
        let output = runtime.format("en", "home.title", &args).expect("format");
        assert_eq!(output, "hi");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_from_parts_formats_message() {
        let runtime = Runtime::from_parts(runtime_parts_fixture("home.title", "hi")).expect("load");
        let args = Args::new();
        let output = runtime.format("en", "home.title", &args).expect("format");
        assert_eq!(output, "hi");
    }

    #[test]
    fn runtime_from_parts_rejects_missing_pack() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts.packs.remove("en");

        let err = runtime_from_parts_err(parts);
        assert!(matches!(err, RuntimeError::MissingPack(locale) if locale == "en"));
    }

    #[test]
    fn runtime_from_parts_rejects_bad_pack_hash() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts.manifest.mf2_packs.get_mut("en").expect("pack").hash =
            format!("sha256:{}", "00".repeat(32));

        let err = runtime_from_parts_err(parts);
        assert!(matches!(err, RuntimeError::PackHashMismatch(locale) if locale == "en"));
    }

    #[test]
    fn runtime_from_parts_rejects_bad_pack_size() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts.manifest.mf2_packs.get_mut("en").expect("pack").size += 1;

        let err = runtime_from_parts_err(parts);
        assert!(matches!(err, RuntimeError::PackSizeMismatch(locale) if locale == "en"));
    }

    #[test]
    fn runtime_from_parts_rejects_bad_pack_schema() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts
            .manifest
            .mf2_packs
            .get_mut("en")
            .expect("pack")
            .pack_schema = 1;

        let err = runtime_from_parts_err(parts);
        assert!(matches!(err, RuntimeError::PackSchemaMismatch(locale) if locale == "en"));
    }

    #[test]
    fn runtime_from_parts_rejects_pack_locale_mismatch() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        let id_map = IdMap::from_bytes(&parts.id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash, "fr", None, PackKind::Base, "hi");
        let pack = parts.manifest.mf2_packs.get_mut("en").expect("pack");
        pack.hash = format!("sha256:{}", hex::encode(super::sha256(&pack_bytes)));
        pack.size = pack_bytes.len() as u64;
        parts.packs.insert("en".to_string(), pack_bytes);

        let err = runtime_from_parts_err(parts);
        assert!(matches!(
            err,
            RuntimeError::PackLocaleMismatch { expected, actual }
                if expected == "en" && actual == "fr"
        ));
    }

    #[test]
    fn runtime_from_parts_rejects_invalid_locale_tag() {
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts.manifest.supported_locales.push("en_us".to_string());

        let err = runtime_from_parts_err(parts);
        assert!(
            matches!(err, RuntimeError::Core(message) if message.contains("invalid language subtag"))
        );
    }

    #[test]
    fn runtime_from_parts_supports_micro_locale_parent_pack() {
        let id_map_json = r#"{"home.title": 0}"#.as_bytes().to_vec();
        let id_map = IdMap::from_bytes(&id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let base_bytes = build_pack_bytes(id_map_hash, "en", None, PackKind::Base, "base");
        let overlay_bytes = build_pack_bytes(
            id_map_hash,
            "en-x-test",
            Some("en"),
            PackKind::Overlay,
            "test",
        );
        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            pack_entry("base", "en", None, &base_bytes),
        );
        mf2_packs.insert(
            "en-x-test".to_string(),
            pack_entry("overlay", "en-x-test", Some("en"), &overlay_bytes),
        );
        let mut micro_locales = BTreeMap::new();
        micro_locales.insert("en-x-test".to_string(), "en".to_string());
        let mut packs = BTreeMap::new();
        packs.insert("en".to_string(), base_bytes);
        packs.insert("en-x-test".to_string(), overlay_bytes);
        let manifest = Manifest {
            schema: 1,
            release_id: "r1".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            supported_locales: vec!["en".to_string(), "en-x-test".to_string()],
            id_map_hash: format!("sha256:{}", hex::encode(id_map_hash)),
            mf2_packs,
            icu_packs: None,
            micro_locales: Some(micro_locales),
            budgets: None,
            signing: None,
        };

        let runtime =
            Runtime::from_parts(RuntimeParts::new(manifest, id_map_json, packs)).expect("runtime");
        let output = runtime
            .format_with_backend("en-x-test", "home.title", &Args::new(), &BasicFormatBackend)
            .expect("format");
        assert_eq!(output, "test");
    }

    #[test]
    fn load_from_paths_reuses_in_memory_validation() {
        let root = temp_dir();
        let mut parts = runtime_parts_fixture("home.title", "hi");
        parts.manifest.mf2_packs.get_mut("en").expect("pack").size += 1;
        let (manifest_path, id_map_path) = write_runtime_parts(&root, &parts);

        let err = match Runtime::load_from_paths(&manifest_path, &id_map_path) {
            Ok(_) => panic!("runtime should fail"),
            Err(err) => err,
        };
        assert!(matches!(err, RuntimeError::PackSizeMismatch(locale) if locale == "en"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_uses_std_backend_for_number_formatter() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.total", "{ $count:number }");
        let mut args = Args::new();
        args.insert("count", Value::Num(12345.5));

        let output = runtime.format("en", "home.total", &args).expect("format");
        assert_eq!(output, "12,345.5");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_uses_std_backend_for_currency_formatter() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.total", "{ $total:currency }");
        let mut args = Args::new();
        args.insert(
            "total",
            Value::Currency {
                value: 12345.5,
                code: *b"USD",
            },
        );

        let output = runtime.format("en", "home.total", &args).expect("format");
        assert_eq!(output, "USD 12,345.5");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_rejects_unit_formatter_without_label_data() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.distance", "{ $distance:unit }");
        let mut args = Args::new();
        args.insert(
            "distance",
            Value::Unit {
                value: 12.5,
                unit_id: 7,
            },
        );

        let err = runtime
            .format("en", "home.distance", &args)
            .expect_err("unit formatter should fail");
        assert_eq!(
            err.to_string(),
            "core error: unsupported: unit formatting requires unit label data"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_reports_missing_date_locale_data() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.when", "{ $when:date }");
        let mut args = Args::new();
        args.insert(
            "when",
            Value::DateTime(DateTimeValue::unix_seconds(994550400)),
        );

        let err = runtime
            .format("haw-US", "home.when", &args)
            .expect_err("date formatter should fail");
        assert_eq!(
            err.to_string(),
            "core error: unsupported: date formatting data unavailable for locale"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_uses_basic_backend_when_requested() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.total", "{ $count:number }");
        let mut args = Args::new();
        args.insert("count", Value::Num(3.5));

        let output = runtime
            .format_with_backend("en", "home.total", &args, &BasicFormatBackend)
            .expect("format ok");
        assert_eq!(output, "3.5");

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_plural_uses_std_backend_when_exact_match_is_absent() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(
            &root,
            "home.count",
            "{ $count:plural -> [one] {one} *[other] {other} }",
        );
        let mut other = Args::new();
        other.insert("count", Value::Num(2.0));

        assert_eq!(
            runtime.format("en", "home.count", &other).expect("other"),
            "other"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_datetime_format_is_explicit_about_seconds_and_milliseconds() {
        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.when", "{ $when:datetime }");

        let mut seconds_args = Args::new();
        seconds_args.insert(
            "when",
            Value::DateTime(DateTimeValue::unix_seconds(994550400)),
        );

        let mut milliseconds_args = Args::new();
        milliseconds_args.insert(
            "when",
            Value::DateTime(DateTimeValue::unix_milliseconds(994550400000)),
        );

        let seconds = runtime
            .format("en-US", "home.when", &seconds_args)
            .expect("seconds");
        let milliseconds = runtime
            .format("en-US", "home.when", &milliseconds_args)
            .expect("milliseconds");

        assert_eq!(seconds, milliseconds);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runtime_format_with_backend_passes_formatter_options() {
        struct TestBackend;

        impl FormatBackend for TestBackend {
            fn plural_category(&self, _value: f64) -> mf2_i18n_core::CoreResult<PluralCategory> {
                Ok(PluralCategory::Other)
            }

            fn format_number(
                &self,
                value: f64,
                options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                let style = options
                    .iter()
                    .find(|option| option.key == "style")
                    .and_then(|option| match &option.value {
                        FormatterOptionValue::Str(value) => Some(value.as_str()),
                        _ => None,
                    })
                    .unwrap_or("plain");
                Ok(format!("num:{value}:{style}"))
            }

            fn format_date(
                &self,
                value: DateTimeValue,
                _options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_time(
                &self,
                value: DateTimeValue,
                _options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_datetime(
                &self,
                value: DateTimeValue,
                _options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_unit(
                &self,
                value: f64,
                unit_id: u32,
                _options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                Ok(format!("{value}:{unit_id}"))
            }

            fn format_currency(
                &self,
                value: f64,
                code: [u8; 3],
                _options: &[FormatterOption],
            ) -> mf2_i18n_core::CoreResult<String> {
                let code = core::str::from_utf8(&code).unwrap_or("???");
                Ok(format!("{value}:{code}"))
            }
        }

        let root = temp_dir();
        let runtime = write_runtime_fixture(&root, "home.total", "{ $count:number style=percent }");
        let mut args = Args::new();
        args.insert("count", Value::Num(3.5));

        let output = runtime
            .format_with_backend("en", "home.total", &args, &TestBackend)
            .expect("format");
        assert_eq!(output, "num:3.5:percent");

        fs::remove_dir_all(&root).ok();
    }

    fn write_runtime_fixture(root: &PathBuf, key: &str, source: &str) -> Runtime {
        let parts = runtime_parts_fixture(key, source);
        let (manifest_path, id_map_path) = write_runtime_parts(root, &parts);

        Runtime::load_from_paths(&manifest_path, &id_map_path).expect("runtime")
    }

    fn runtime_parts_fixture(key: &str, source: &str) -> RuntimeParts {
        let id_map_json = format!(r#"{{"{key}": 0}}"#).into_bytes();
        let id_map = IdMap::from_bytes(&id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash, "en", None, PackKind::Base, source);
        let mut mf2_packs = BTreeMap::new();
        mf2_packs.insert(
            "en".to_string(),
            pack_entry("base", "en", None, &pack_bytes),
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
        let mut packs = BTreeMap::new();
        packs.insert("en".to_string(), pack_bytes);
        RuntimeParts::new(manifest, id_map_json, packs)
    }

    fn write_runtime_parts(root: &PathBuf, parts: &RuntimeParts) -> (PathBuf, PathBuf) {
        let packs_dir = root.join("packs");
        fs::create_dir_all(&packs_dir).expect("packs");
        for (locale, bytes) in &parts.packs {
            fs::write(packs_dir.join(format!("{locale}.mf2pack")), bytes).expect("write pack");
        }
        let manifest_path = root.join("manifest.json");
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&parts.manifest).expect("json"),
        )
        .expect("write manifest");

        let id_map_path = root.join("id_map.json");
        fs::write(&id_map_path, &parts.id_map_json).expect("write id map");
        (manifest_path, id_map_path)
    }

    fn pack_entry(kind: &str, locale: &str, parent: Option<&str>, bytes: &[u8]) -> PackEntry {
        PackEntry {
            kind: kind.to_string(),
            url: format!("packs/{locale}.mf2pack"),
            hash: format!("sha256:{}", hex::encode(super::sha256(bytes))),
            size: bytes.len() as u64,
            content_encoding: "identity".to_string(),
            pack_schema: 0,
            parent: parent.map(str::to_string),
        }
    }

    fn runtime_from_parts_err(parts: RuntimeParts) -> RuntimeError {
        match Runtime::from_parts(parts) {
            Ok(_) => panic!("runtime should fail"),
            Err(err) => err,
        }
    }
}

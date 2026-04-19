use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mfs_i18n_core::{
    Args, CatalogChain, DateTimeValue, FormatBackend, LanguageTag, PackCatalog, PluralCategory,
    execute, negotiate_lookup,
};
use mfs_i18n_std::StdFormatBackend;

use crate::error::{RuntimeError, RuntimeResult};
use crate::id_map::IdMap;
use crate::loader::{load_id_map, load_manifest, parse_sha256};
use crate::manifest::PackEntry;

pub struct Runtime {
    id_map: IdMap,
    packs: BTreeMap<String, PackCatalog>,
    parents: BTreeMap<String, String>,
    default_locale: LanguageTag,
    supported: Vec<LanguageTag>,
}

pub struct BasicFormatBackend;
pub struct UnsupportedFormatBackend;

impl FormatBackend for BasicFormatBackend {
    fn plural_category(&self, _value: f64) -> mfs_i18n_core::CoreResult<PluralCategory> {
        Ok(PluralCategory::Other)
    }

    fn format_number(
        &self,
        value: f64,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_date(
        &self,
        value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_time(
        &self,
        value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_datetime(
        &self,
        value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_unit(
        &self,
        value: f64,
        unit_id: u32,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Ok(format!("{value}:{unit_id}"))
    }

    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        let code = core::str::from_utf8(&code).unwrap_or("???");
        Ok(format!("{value}:{code}"))
    }
}

impl FormatBackend for UnsupportedFormatBackend {
    fn plural_category(&self, _value: f64) -> mfs_i18n_core::CoreResult<PluralCategory> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "plural selection requires a format backend",
        ))
    }

    fn format_number(
        &self,
        _value: f64,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "number formatting requires a format backend",
        ))
    }

    fn format_date(
        &self,
        _value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "date formatting requires a format backend",
        ))
    }

    fn format_time(
        &self,
        _value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "time formatting requires a format backend",
        ))
    }

    fn format_datetime(
        &self,
        _value: DateTimeValue,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "datetime formatting requires a format backend",
        ))
    }

    fn format_unit(
        &self,
        _value: f64,
        _unit_id: u32,
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "unit formatting requires a format backend",
        ))
    }

    fn format_currency(
        &self,
        _value: f64,
        _code: [u8; 3],
        _options: &[mfs_i18n_core::FormatterOption],
    ) -> mfs_i18n_core::CoreResult<String> {
        Err(mfs_i18n_core::CoreError::Unsupported(
            "currency formatting requires a format backend",
        ))
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
                catalogs.push(pack as &dyn mfs_i18n_core::Catalog);
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
    use super::{BasicFormatBackend, Runtime};
    use crate::id_map::IdMap;
    use crate::manifest::{Manifest, PackEntry};
    use mfs_i18n_build::compiler::compile_message;
    use mfs_i18n_build::pack_encode::{PackBuildInput, encode_pack};
    use mfs_i18n_build::parser::parse_message;
    use mfs_i18n_core::{
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
        path.push(format!("mfs_i18n_runtime_{nanos}"));
        fs::create_dir_all(&path).expect("dir");
        path
    }

    fn build_pack_bytes(id_map_hash: [u8; 32], source: &str) -> Vec<u8> {
        let message = parse_message(source).expect("parse");
        let compiled = compile_message(&message).expect("compile");
        let mut messages = BTreeMap::new();
        messages.insert(MessageId::new(0), compiled.program);
        encode_pack(&PackBuildInput {
            pack_kind: PackKind::Base,
            id_map_hash,
            locale_tag: "en".to_string(),
            parent_tag: None,
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
            fn plural_category(&self, _value: f64) -> mfs_i18n_core::CoreResult<PluralCategory> {
                Ok(PluralCategory::Other)
            }

            fn format_number(
                &self,
                value: f64,
                options: &[FormatterOption],
            ) -> mfs_i18n_core::CoreResult<String> {
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
            ) -> mfs_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_time(
                &self,
                value: DateTimeValue,
                _options: &[FormatterOption],
            ) -> mfs_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_datetime(
                &self,
                value: DateTimeValue,
                _options: &[FormatterOption],
            ) -> mfs_i18n_core::CoreResult<String> {
                Ok(value.to_string())
            }

            fn format_unit(
                &self,
                value: f64,
                unit_id: u32,
                _options: &[FormatterOption],
            ) -> mfs_i18n_core::CoreResult<String> {
                Ok(format!("{value}:{unit_id}"))
            }

            fn format_currency(
                &self,
                value: f64,
                code: [u8; 3],
                _options: &[FormatterOption],
            ) -> mfs_i18n_core::CoreResult<String> {
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
        let packs_dir = root.join("packs");
        fs::create_dir_all(&packs_dir).expect("packs");

        let id_map_json = format!(r#"{{"{key}": 0}}"#);
        let id_map = IdMap::from_json(&id_map_json).expect("id map");
        let id_map_hash = id_map.hash().expect("hash");
        let pack_bytes = build_pack_bytes(id_map_hash, source);
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
        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).expect("json"),
        )
        .expect("write manifest");

        let id_map_path = root.join("id_map.json");
        fs::write(&id_map_path, id_map_json).expect("write id map");

        Runtime::load_from_paths(&manifest_path, &id_map_path).expect("runtime")
    }
}

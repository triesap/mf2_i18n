#![allow(clippy::too_many_arguments)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use mf2_i18n_core::{
    Args, Catalog, CoreError, CoreResult, FormatBackend, LanguageTag, PackCatalog, PluralCategory,
    execute, negotiate_lookup,
};

pub struct EmbeddedPack<'a> {
    pub locale: &'a str,
    pub bytes: &'a [u8],
}

pub struct EmbeddedRuntime {
    id_map: BTreeMap<String, mf2_i18n_core::MessageId>,
    packs: BTreeMap<String, PackCatalog>,
    default_locale: LanguageTag,
    supported: Vec<LanguageTag>,
}

pub struct BasicFormatBackend;
pub struct UnsupportedFormatBackend;

impl FormatBackend for BasicFormatBackend {
    fn plural_category(&self, _value: f64) -> CoreResult<PluralCategory> {
        Ok(PluralCategory::Other)
    }

    fn format_number(
        &self,
        value: f64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_date(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_time(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_datetime(
        &self,
        value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_unit(
        &self,
        value: f64,
        unit_id: u32,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Ok(alloc::format!("{value}:{unit_id}"))
    }

    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        let code = core::str::from_utf8(&code).unwrap_or("???");
        Ok(alloc::format!("{value}:{code}"))
    }
}

impl FormatBackend for UnsupportedFormatBackend {
    fn plural_category(&self, _value: f64) -> CoreResult<PluralCategory> {
        Err(CoreError::Unsupported(
            "plural selection requires a format backend",
        ))
    }

    fn format_number(
        &self,
        _value: f64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "number formatting requires a format backend",
        ))
    }

    fn format_date(
        &self,
        _value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "date formatting requires a format backend",
        ))
    }

    fn format_time(
        &self,
        _value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "time formatting requires a format backend",
        ))
    }

    fn format_datetime(
        &self,
        _value: i64,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "datetime formatting requires a format backend",
        ))
    }

    fn format_unit(
        &self,
        _value: f64,
        _unit_id: u32,
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "unit formatting requires a format backend",
        ))
    }

    fn format_currency(
        &self,
        _value: f64,
        _code: [u8; 3],
        _options: &[mf2_i18n_core::FormatterOption],
    ) -> CoreResult<String> {
        Err(CoreError::Unsupported(
            "currency formatting requires a format backend",
        ))
    }
}

impl EmbeddedRuntime {
    pub fn new(
        id_map: BTreeMap<String, mf2_i18n_core::MessageId>,
        id_map_hash: [u8; 32],
        packs: &[EmbeddedPack<'_>],
        default_locale: &str,
    ) -> CoreResult<Self> {
        let mut pack_map = BTreeMap::new();
        let mut supported = Vec::new();
        for pack in packs {
            let catalog = PackCatalog::decode(pack.bytes, &id_map_hash)?;
            pack_map.insert(pack.locale.to_string(), catalog);
            supported.push(LanguageTag::parse(pack.locale)?);
        }
        let default_locale = LanguageTag::parse(default_locale)?;
        Ok(Self {
            id_map,
            packs: pack_map,
            default_locale,
            supported,
        })
    }

    pub fn format(&self, locale: &str, key: &str, args: &Args) -> CoreResult<String> {
        let backend = UnsupportedFormatBackend;
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
    ) -> CoreResult<String> {
        let locale_tag = LanguageTag::parse(locale)?;
        let negotiation = negotiate_lookup(&[locale_tag], &self.supported, &self.default_locale);
        let selected = negotiation.selected.normalized();

        let catalog = self
            .packs
            .get(selected)
            .ok_or(CoreError::InvalidInput("missing locale"))?;
        let message_id = self
            .id_map
            .get(key)
            .copied()
            .ok_or(CoreError::InvalidInput("missing message"))?;
        let program = catalog
            .lookup(message_id)
            .ok_or(CoreError::InvalidInput("missing message"))?;
        execute(program, args, backend)
    }
}

#[cfg(test)]
mod tests {
    use super::{BasicFormatBackend, EmbeddedPack, EmbeddedRuntime};
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;
    use alloc::vec::Vec;
    use mf2_i18n_build::compiler::compile_message;
    use mf2_i18n_build::pack_encode::{PackBuildInput, encode_pack};
    use mf2_i18n_build::parser::parse_message;
    use mf2_i18n_core::{Args, MessageId, PackKind, Value};
    use mf2_i18n_std::StdFormatBackend;

    fn build_pack_bytes(id_map_hash: [u8; 32], locale_tag: &str, source: &str) -> Vec<u8> {
        let message = parse_message(source).expect("parse");
        let compiled = compile_message(&message);
        let mut messages = BTreeMap::new();
        messages.insert(MessageId::new(0), compiled.program);
        encode_pack(&PackBuildInput {
            pack_kind: PackKind::Base,
            id_map_hash,
            locale_tag: locale_tag.to_string(),
            parent_tag: None,
            build_epoch_ms: 0,
            messages,
        })
    }

    #[test]
    fn formats_with_embedded_runtime() {
        let runtime = build_runtime("home.title", "hi");
        let args = Args::new();
        let output = runtime.format("en", "home.title", &args).expect("format");
        assert_eq!(output, "hi");
    }

    #[test]
    fn embedded_runtime_requires_backend_for_number_formatter() {
        let runtime = build_runtime("home.total", "{ $count:number }");
        let mut args = Args::new();
        args.insert("count", Value::Num(3.5));

        let err = runtime
            .format("en", "home.total", &args)
            .expect_err("default formatter should fail");
        assert_eq!(
            err.to_string(),
            "unsupported: number formatting requires a format backend"
        );
    }

    #[test]
    fn embedded_runtime_uses_basic_backend_when_requested() {
        let runtime = build_runtime("home.total", "{ $count:number }");
        let mut args = Args::new();
        args.insert("count", Value::Num(3.5));

        let output = runtime
            .format_with_backend("en", "home.total", &args, &BasicFormatBackend)
            .expect("format");
        assert_eq!(output, "3.5");
    }

    #[test]
    fn embedded_runtime_uses_std_backend_when_requested() {
        let runtime = build_runtime_for_locale("fr", "home.total", "{ $count:number }");
        let mut args = Args::new();
        args.insert("count", Value::Num(12345.5));
        let backend = StdFormatBackend::new("fr-BE").expect("backend");

        let output = runtime
            .format_with_backend("fr-BE", "home.total", &args, &backend)
            .expect("format");
        assert_eq!(output, "12\u{202f}345,5");
    }

    #[test]
    fn embedded_plural_requires_backend_when_exact_match_is_absent() {
        let runtime = build_runtime(
            "home.count",
            "{ $count:plural -> [one] {one} *[other] {other} }",
        );
        let mut args = Args::new();
        args.insert("count", Value::Num(2.0));

        let err = runtime
            .format("en", "home.count", &args)
            .expect_err("default plural selection should fail");
        assert_eq!(
            err.to_string(),
            "unsupported: plural selection requires a format backend"
        );
    }

    fn build_runtime(key: &str, source: &str) -> EmbeddedRuntime {
        build_runtime_for_locale("en", key, source)
    }

    fn build_runtime_for_locale(locale: &str, key: &str, source: &str) -> EmbeddedRuntime {
        let mut id_map = BTreeMap::new();
        id_map.insert(key.to_string(), MessageId::new(0));
        let id_map_hash = [7u8; 32];
        let pack_bytes = build_pack_bytes(id_map_hash, locale, source);
        let packs = [EmbeddedPack {
            locale,
            bytes: &pack_bytes,
        }];
        EmbeddedRuntime::new(id_map, id_map_hash, &packs, locale).expect("runtime")
    }
}

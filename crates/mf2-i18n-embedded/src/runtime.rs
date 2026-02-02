#![allow(clippy::too_many_arguments)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use mf2_i18n_core::{
    execute, negotiate_lookup, Args, Catalog, CoreError, CoreResult, FormatBackend, LanguageTag,
    PackCatalog, PluralCategory,
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

impl FormatBackend for BasicFormatBackend {
    fn plural_category(&self, _value: f64) -> CoreResult<PluralCategory> {
        Ok(PluralCategory::Other)
    }

    fn format_number(&self, value: f64, _options: &[mf2_i18n_core::FormatterOption]) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_date(&self, value: i64, _options: &[mf2_i18n_core::FormatterOption]) -> CoreResult<String> {
        Ok(value.to_string())
    }

    fn format_time(&self, value: i64, _options: &[mf2_i18n_core::FormatterOption]) -> CoreResult<String> {
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
        let backend = BasicFormatBackend;
        self.format_with_backend(locale, key, args, &backend)
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
    use super::{EmbeddedPack, EmbeddedRuntime};
    use alloc::string::ToString;
    use alloc::vec;
    use alloc::vec::Vec;
    use alloc::collections::BTreeMap;
    use mf2_i18n_core::{Args, MessageId, PackKind};

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

    #[test]
    fn formats_with_embedded_runtime() {
        let mut id_map = BTreeMap::new();
        id_map.insert("home.title".to_string(), MessageId::new(0));
        let id_map_hash = [7u8; 32];
        let pack_bytes = build_pack_bytes(id_map_hash);
        let packs = [EmbeddedPack {
            locale: "en",
            bytes: &pack_bytes,
        }];
        let runtime = EmbeddedRuntime::new(id_map, id_map_hash, &packs, "en").expect("runtime");
        let args = Args::new();
        let output = runtime.format("en", "home.title", &args).expect("format");
        assert_eq!(output, "hi");
    }
}

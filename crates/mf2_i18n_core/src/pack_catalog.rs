use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    decode_dense_index, decode_sparse_index, decode_string_pool, parse_pack_header,
    parse_section_directory, read_bytecode_at, BytecodeProgram, CaseEntry, CaseKey, CaseTable,
    Catalog, CoreError, CoreResult, FormatterId, MessageId, PackHeader, PackKind, PluralRuleset,
    SectionEntry, StringPool,
};

const SECTION_STRING_POOL: u8 = 1;
const SECTION_MESSAGE_INDEX: u8 = 2;
const SECTION_BYTECODE_BLOB: u8 = 3;
const SECTION_CASE_TABLES: u8 = 4;
const SECTION_MESSAGE_META: u8 = 5;

pub struct PackCatalog {
    header: PackHeader,
    messages: BTreeMap<MessageId, BytecodeProgram>,
}

impl PackCatalog {
    pub fn decode(bytes: &[u8], expected_id_map_hash: &[u8; 32]) -> CoreResult<Self> {
        let (header, mut cursor) = parse_pack_header(bytes)?;
        if &header.id_map_hash != expected_id_map_hash {
            return Err(CoreError::InvalidInput("id map hash mismatch"));
        }
        let section_count = read_u16(bytes, &mut cursor)? as usize;
        let sections = parse_section_directory(bytes, cursor, section_count)?;
        let section_map = map_sections(bytes, &sections)?;

        let string_pool_bytes = section_map
            .get(&SECTION_STRING_POOL)
            .ok_or(CoreError::InvalidInput("missing string pool section"))?;
        let string_pool = decode_string_pool(string_pool_bytes)?;

        let case_tables_bytes = section_map
            .get(&SECTION_CASE_TABLES)
            .ok_or(CoreError::InvalidInput("missing case tables section"))?;
        let case_tables = decode_case_tables(case_tables_bytes)?;

        let meta_bytes = section_map
            .get(&SECTION_MESSAGE_META)
            .ok_or(CoreError::InvalidInput("missing message meta section"))?;
        let meta = decode_message_meta(meta_bytes, &string_pool)?;

        let index_bytes = section_map
            .get(&SECTION_MESSAGE_INDEX)
            .ok_or(CoreError::InvalidInput("missing message index section"))?;
        let index = match header.pack_kind {
            PackKind::Base => MessageIndex::Dense(decode_dense_index(index_bytes)?),
            PackKind::Overlay => MessageIndex::Sparse(decode_sparse_index(index_bytes)?),
            PackKind::IcuData => return Err(CoreError::Unsupported("icu data packs not supported")),
        };

        let blob = section_map
            .get(&SECTION_BYTECODE_BLOB)
            .ok_or(CoreError::InvalidInput("missing bytecode blob section"))?;

        let mut messages = BTreeMap::new();
        match index {
            MessageIndex::Dense(offsets) => {
                for (id, offset) in offsets.into_iter().enumerate() {
                    if offset == u32::MAX {
                        continue;
                    }
                    let message_id = MessageId::new(id as u32);
                    let slice = read_bytecode_at(blob, offset)?;
                    let arg_names = meta.get(&message_id).cloned().unwrap_or_default();
                    let program =
                        decode_message(slice, &string_pool, &case_tables, arg_names)?;
                    messages.insert(message_id, program);
                }
            }
            MessageIndex::Sparse(pairs) => {
                for (message_id, offset) in pairs {
                    let slice = read_bytecode_at(blob, offset)?;
                    let arg_names = meta.get(&message_id).cloned().unwrap_or_default();
                    let program =
                        decode_message(slice, &string_pool, &case_tables, arg_names)?;
                    messages.insert(message_id, program);
                }
            }
        }

        Ok(Self { header, messages })
    }

    pub fn header(&self) -> &PackHeader {
        &self.header
    }
}

impl Catalog for PackCatalog {
    fn lookup(&self, id: MessageId) -> Option<&BytecodeProgram> {
        self.messages.get(&id)
    }
}

enum MessageIndex {
    Dense(Vec<u32>),
    Sparse(Vec<(MessageId, u32)>),
}

fn map_sections<'a>(
    bytes: &'a [u8],
    sections: &[SectionEntry],
) -> CoreResult<BTreeMap<u8, &'a [u8]>> {
    let mut map = BTreeMap::new();
    for section in sections {
        let start = section.offset as usize;
        let end = start + section.length as usize;
        if end > bytes.len() {
            return Err(CoreError::InvalidInput("section out of bounds"));
        }
        map.insert(section.section_type, &bytes[start..end]);
    }
    Ok(map)
}

fn decode_case_tables(input: &[u8]) -> CoreResult<Vec<CaseTable>> {
    let mut cursor = 0usize;
    let count = read_u32(input, &mut cursor)? as usize;
    let mut tables = Vec::with_capacity(count);
    for _ in 0..count {
        let entry_count = read_u32(input, &mut cursor)? as usize;
        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let key_type = read_u8(input, &mut cursor)?;
            let key = match key_type {
                0 => CaseKey::String(read_u32(input, &mut cursor)?),
                1 => CaseKey::Exact(read_u32(input, &mut cursor)?),
                2 => {
                    let raw = read_u8(input, &mut cursor)?;
                    let category = match raw {
                        0 => crate::PluralCategory::Zero,
                        1 => crate::PluralCategory::One,
                        2 => crate::PluralCategory::Two,
                        3 => crate::PluralCategory::Few,
                        4 => crate::PluralCategory::Many,
                        _ => crate::PluralCategory::Other,
                    };
                    CaseKey::Category(category)
                }
                3 => CaseKey::Other,
                _ => return Err(CoreError::InvalidInput("unknown case key type")),
            };
            let target = read_u32(input, &mut cursor)?;
            entries.push(CaseEntry { key, target });
        }
        tables.push(CaseTable { entries });
    }
    Ok(tables)
}

fn decode_message_meta(
    input: &[u8],
    string_pool: &[String],
) -> CoreResult<BTreeMap<MessageId, Vec<String>>> {
    let mut cursor = 0usize;
    let count = read_u32(input, &mut cursor)? as usize;
    let mut map = BTreeMap::new();
    for _ in 0..count {
        let id = read_u32(input, &mut cursor)?;
        let arg_count = read_u32(input, &mut cursor)? as usize;
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            let sidx = read_u32(input, &mut cursor)? as usize;
            let name = string_pool
                .get(sidx)
                .ok_or(CoreError::InvalidInput("message meta string index"))?;
            args.push(name.clone());
        }
        map.insert(MessageId::new(id), args);
    }
    Ok(map)
}

fn decode_message(
    input: &[u8],
    string_pool: &[String],
    case_tables: &[CaseTable],
    arg_names: Vec<String>,
) -> CoreResult<BytecodeProgram> {
    let mut cursor = 0usize;
    let number_count = read_u32(input, &mut cursor)? as usize;
    let mut number_pool = Vec::with_capacity(number_count);
    for _ in 0..number_count {
        number_pool.push(read_f64(input, &mut cursor)?);
    }
    let opcode_count = read_u32(input, &mut cursor)? as usize;
    let mut opcodes = Vec::with_capacity(opcode_count);
    for _ in 0..opcode_count {
        let tag = read_u8(input, &mut cursor)?;
        let opcode = match tag {
            0 => crate::Opcode::EmitText {
                sidx: read_u32(input, &mut cursor)?,
            },
            1 => crate::Opcode::EmitStack,
            2 => crate::Opcode::PushStr {
                sidx: read_u32(input, &mut cursor)?,
            },
            3 => crate::Opcode::PushNum {
                nidx: read_u32(input, &mut cursor)?,
            },
            4 => crate::Opcode::PushArg {
                aidx: read_u32(input, &mut cursor)?,
            },
            5 => crate::Opcode::Dup,
            6 => crate::Opcode::Pop,
            7 => {
                let fid = FormatterId::try_from(read_u8(input, &mut cursor)?)?;
                let opt_count = read_u8(input, &mut cursor)?;
                crate::Opcode::CallFmt { fid, opt_count }
            }
            8 => crate::Opcode::Select {
                aidx: read_u32(input, &mut cursor)?,
                table: read_u32(input, &mut cursor)?,
            },
            9 => {
                let aidx = read_u32(input, &mut cursor)?;
                let ruleset = PluralRuleset::try_from(read_u8(input, &mut cursor)?)?;
                let table = read_u32(input, &mut cursor)?;
                crate::Opcode::SelectPlural {
                    aidx,
                    ruleset,
                    table,
                }
            }
            10 => crate::Opcode::Jump {
                rel: read_i32(input, &mut cursor)?,
            },
            11 => crate::Opcode::End,
            _ => return Err(CoreError::InvalidInput("unknown opcode tag")),
        };
        opcodes.push(opcode);
    }

    let mut pool = StringPool::new();
    for entry in string_pool {
        pool.push(entry.clone());
    }
    let mut program = BytecodeProgram::new();
    program.opcodes = opcodes;
    program.number_pool = number_pool;
    program.case_tables = case_tables.to_vec();
    program.string_pool = pool;
    program.arg_names = arg_names;
    Ok(program)
}

fn read_u8(input: &[u8], cursor: &mut usize) -> CoreResult<u8> {
    let end = *cursor + 1;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = input[*cursor];
    *cursor = end;
    Ok(value)
}

fn read_u16(input: &[u8], cursor: &mut usize) -> CoreResult<u16> {
    let end = *cursor + 2;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = u16::from_le_bytes([input[*cursor], input[*cursor + 1]]);
    *cursor = end;
    Ok(value)
}

fn read_u32(input: &[u8], cursor: &mut usize) -> CoreResult<u32> {
    let end = *cursor + 4;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = u32::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
    ]);
    *cursor = end;
    Ok(value)
}

fn read_i32(input: &[u8], cursor: &mut usize) -> CoreResult<i32> {
    let end = *cursor + 4;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = i32::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
    ]);
    *cursor = end;
    Ok(value)
}

fn read_f64(input: &[u8], cursor: &mut usize) -> CoreResult<f64> {
    let end = *cursor + 8;
    if end > input.len() {
        return Err(CoreError::InvalidInput("unexpected eof"));
    }
    let value = f64::from_le_bytes([
        input[*cursor],
        input[*cursor + 1],
        input[*cursor + 2],
        input[*cursor + 3],
        input[*cursor + 4],
        input[*cursor + 5],
        input[*cursor + 6],
        input[*cursor + 7],
    ]);
    *cursor = end;
    Ok(value)
}

impl TryFrom<u8> for FormatterId {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FormatterId::Number),
            1 => Ok(FormatterId::Date),
            2 => Ok(FormatterId::Time),
            3 => Ok(FormatterId::DateTime),
            4 => Ok(FormatterId::Unit),
            5 => Ok(FormatterId::Currency),
            6 => Ok(FormatterId::Identity),
            _ => Err(CoreError::InvalidInput("unknown formatter id")),
        }
    }
}

impl TryFrom<u8> for PluralRuleset {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PluralRuleset::Cardinal),
            _ => Err(CoreError::InvalidInput("unknown plural ruleset")),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use alloc::vec::Vec;

    use super::{PackCatalog, SECTION_BYTECODE_BLOB, SECTION_CASE_TABLES, SECTION_MESSAGE_INDEX, SECTION_MESSAGE_META, SECTION_STRING_POOL};
    use crate::{Catalog, MessageId, Opcode, PackKind};

    fn build_header(kind: PackKind, id_map_hash: [u8; 32]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"MF2PACK\0");
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.push(match kind {
            PackKind::Base => 0,
            PackKind::Overlay => 1,
            PackKind::IcuData => 2,
        });
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&id_map_hash);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&u32::MAX.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes
    }

    #[test]
    fn decodes_pack_catalog() {
        let id_map_hash = [7u8; 32];
        let mut bytes = build_header(PackKind::Base, id_map_hash);

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
            (SECTION_STRING_POOL, string_pool),
            (SECTION_MESSAGE_INDEX, message_index),
            (SECTION_BYTECODE_BLOB, bytecode_blob),
            (SECTION_CASE_TABLES, case_tables),
            (SECTION_MESSAGE_META, message_meta),
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

        let catalog = PackCatalog::decode(&bytes, &id_map_hash).expect("catalog");
        let program = catalog.lookup(MessageId::new(0)).expect("program");
        assert_eq!(program.opcodes, vec![Opcode::EmitText { sidx: 0 }, Opcode::End]);
    }
}

use std::collections::BTreeMap;

use mf2_i18n_core::{
    BytecodeProgram, CaseEntry, CaseKey, CaseTable, MessageId, Opcode, PackKind, PluralCategory,
    PluralRuleset, StringPool,
};

pub struct PackBuildInput {
    pub pack_kind: PackKind,
    pub id_map_hash: [u8; 32],
    pub locale_tag: String,
    pub parent_tag: Option<String>,
    pub build_epoch_ms: u64,
    pub messages: BTreeMap<MessageId, BytecodeProgram>,
}

pub fn encode_pack(input: &PackBuildInput) -> Vec<u8> {
    let mut interner = StringInterner::new();
    let locale_tag_sidx = interner.intern(&input.locale_tag);
    let parent_tag_sidx = input.parent_tag.as_ref().map(|tag| interner.intern(tag));

    let mut remapped_messages = BTreeMap::new();
    let mut case_tables = Vec::new();
    for (message_id, program) in &input.messages {
        let (remapped, local_tables) = remap_program(program, &mut interner, case_tables.len() as u32);
        case_tables.extend(local_tables);
        remapped_messages.insert(*message_id, remapped);
    }

    let string_pool = interner.into_pool();
    let string_section = encode_string_pool(&string_pool);
    let case_section = encode_case_tables(&case_tables);
    let meta_section = encode_message_meta(&remapped_messages, &string_pool);
    let (blob_section, index_section) =
        encode_bytecode_blob(&remapped_messages, input.pack_kind);

    let mut sections = Vec::new();
    sections.push((1u8, string_section));
    sections.push((2u8, index_section));
    sections.push((3u8, blob_section));
    sections.push((4u8, case_section));
    sections.push((5u8, meta_section));

    build_pack_bytes(
        input.pack_kind,
        input.id_map_hash,
        locale_tag_sidx,
        parent_tag_sidx,
        input.build_epoch_ms,
        sections,
    )
}

fn remap_program(
    program: &BytecodeProgram,
    interner: &mut StringInterner,
    case_offset: u32,
) -> (BytecodeProgram, Vec<CaseTable>) {
    let mut mapping = Vec::with_capacity(program.string_pool.len());
    for idx in 0..program.string_pool.len() {
        let value = program.string_pool.get(idx as u32).unwrap_or("");
        let new_idx = interner.intern(value);
        mapping.push(new_idx);
    }

    for arg in &program.arg_names {
        interner.intern(arg);
    }

    let mut tables = Vec::with_capacity(program.case_tables.len());
    for table in &program.case_tables {
        let mut entries = Vec::with_capacity(table.entries.len());
        for entry in &table.entries {
            let key = match entry.key {
                CaseKey::String(old) => {
                    let sidx = mapping[old as usize];
                    CaseKey::String(sidx)
                }
                CaseKey::Exact(value) => CaseKey::Exact(value),
                CaseKey::Category(cat) => CaseKey::Category(cat),
                CaseKey::Other => CaseKey::Other,
            };
            entries.push(CaseEntry {
                key,
                target: entry.target,
            });
        }
        tables.push(CaseTable { entries });
    }

    let mut opcodes = Vec::with_capacity(program.opcodes.len());
    for opcode in &program.opcodes {
        let remapped = match *opcode {
            Opcode::EmitText { sidx } => Opcode::EmitText {
                sidx: mapping[sidx as usize],
            },
            Opcode::PushStr { sidx } => Opcode::PushStr {
                sidx: mapping[sidx as usize],
            },
            Opcode::Select { aidx, table } => Opcode::Select {
                aidx,
                table: table + case_offset,
            },
            Opcode::SelectPlural {
                aidx,
                ruleset,
                table,
            } => Opcode::SelectPlural {
                aidx,
                ruleset,
                table: table + case_offset,
            },
            other => other,
        };
        opcodes.push(remapped);
    }

    let mut program_out = BytecodeProgram::new();
    program_out.opcodes = opcodes;
    program_out.number_pool = program.number_pool.clone();
    program_out.case_tables = Vec::new();
    program_out.string_pool = StringPool::new();
    program_out.arg_names = program.arg_names.clone();

    (program_out, tables)
}

fn encode_string_pool(pool: &StringPool) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(pool.len() as u32).to_le_bytes());
    for idx in 0..pool.len() {
        let value = pool.get(idx as u32).unwrap_or("");
        let slice = value.as_bytes();
        bytes.extend_from_slice(&(slice.len() as u32).to_le_bytes());
        bytes.extend_from_slice(slice);
    }
    bytes
}

fn encode_case_tables(tables: &[CaseTable]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(tables.len() as u32).to_le_bytes());
    for table in tables {
        bytes.extend_from_slice(&(table.entries.len() as u32).to_le_bytes());
        for entry in &table.entries {
            match entry.key {
                CaseKey::String(sidx) => {
                    bytes.push(0);
                    bytes.extend_from_slice(&sidx.to_le_bytes());
                }
                CaseKey::Exact(value) => {
                    bytes.push(1);
                    bytes.extend_from_slice(&value.to_le_bytes());
                }
                CaseKey::Category(cat) => {
                    bytes.push(2);
                    bytes.push(encode_category(cat));
                }
                CaseKey::Other => {
                    bytes.push(3);
                }
            }
            bytes.extend_from_slice(&entry.target.to_le_bytes());
        }
    }
    bytes
}

fn encode_message_meta(
    messages: &BTreeMap<MessageId, BytecodeProgram>,
    pool: &StringPool,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(messages.len() as u32).to_le_bytes());
    for (message_id, program) in messages {
        bytes.extend_from_slice(&message_id.get().to_le_bytes());
        bytes.extend_from_slice(&(program.arg_names.len() as u32).to_le_bytes());
        for arg in &program.arg_names {
            let sidx = find_string(pool, arg);
            bytes.extend_from_slice(&sidx.to_le_bytes());
        }
    }
    bytes
}

fn encode_bytecode_blob(
    messages: &BTreeMap<MessageId, BytecodeProgram>,
    pack_kind: PackKind,
) -> (Vec<u8>, Vec<u8>) {
    let mut blob = Vec::new();
    let mut offsets = BTreeMap::new();
    for (message_id, program) in messages {
        let offset = blob.len() as u32;
        let bytes = encode_message(program);
        blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        blob.extend_from_slice(&bytes);
        offsets.insert(*message_id, offset);
    }

    let index = match pack_kind {
        PackKind::Base => encode_dense_index(&offsets),
        PackKind::Overlay => encode_sparse_index(&offsets),
        PackKind::IcuData => Vec::new(),
    };
    (blob, index)
}

fn encode_dense_index(offsets: &BTreeMap<MessageId, u32>) -> Vec<u8> {
    let max_id = offsets.keys().map(|id| id.get()).max().unwrap_or(0);
    let mut bytes = Vec::new();
    let count = max_id + 1;
    bytes.extend_from_slice(&count.to_le_bytes());
    for id in 0..count {
        let value = offsets.get(&MessageId::new(id)).copied().unwrap_or(u32::MAX);
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn encode_sparse_index(offsets: &BTreeMap<MessageId, u32>) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(offsets.len() as u32).to_le_bytes());
    for (id, offset) in offsets {
        bytes.extend_from_slice(&id.get().to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes
}

fn encode_message(program: &BytecodeProgram) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(program.number_pool.len() as u32).to_le_bytes());
    for value in &program.number_pool {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&(program.opcodes.len() as u32).to_le_bytes());
    for opcode in &program.opcodes {
        encode_opcode(&mut bytes, *opcode);
    }
    bytes
}

fn encode_opcode(bytes: &mut Vec<u8>, opcode: Opcode) {
    match opcode {
        Opcode::EmitText { sidx } => {
            bytes.push(0);
            bytes.extend_from_slice(&sidx.to_le_bytes());
        }
        Opcode::EmitStack => bytes.push(1),
        Opcode::PushStr { sidx } => {
            bytes.push(2);
            bytes.extend_from_slice(&sidx.to_le_bytes());
        }
        Opcode::PushNum { nidx } => {
            bytes.push(3);
            bytes.extend_from_slice(&nidx.to_le_bytes());
        }
        Opcode::PushArg { aidx } => {
            bytes.push(4);
            bytes.extend_from_slice(&aidx.to_le_bytes());
        }
        Opcode::Dup => bytes.push(5),
        Opcode::Pop => bytes.push(6),
        Opcode::CallFmt { fid, opt_count } => {
            bytes.push(7);
            bytes.push(fid as u8);
            bytes.push(opt_count);
        }
        Opcode::Select { aidx, table } => {
            bytes.push(8);
            bytes.extend_from_slice(&aidx.to_le_bytes());
            bytes.extend_from_slice(&table.to_le_bytes());
        }
        Opcode::SelectPlural {
            aidx,
            ruleset,
            table,
        } => {
            bytes.push(9);
            bytes.extend_from_slice(&aidx.to_le_bytes());
            bytes.push(encode_ruleset(ruleset));
            bytes.extend_from_slice(&table.to_le_bytes());
        }
        Opcode::Jump { rel } => {
            bytes.push(10);
            bytes.extend_from_slice(&rel.to_le_bytes());
        }
        Opcode::End => bytes.push(11),
    }
}

fn encode_ruleset(ruleset: PluralRuleset) -> u8 {
    match ruleset {
        PluralRuleset::Cardinal => 0,
    }
}

fn encode_category(category: PluralCategory) -> u8 {
    match category {
        PluralCategory::Zero => 0,
        PluralCategory::One => 1,
        PluralCategory::Two => 2,
        PluralCategory::Few => 3,
        PluralCategory::Many => 4,
        PluralCategory::Other => 5,
    }
}

fn find_string(pool: &StringPool, value: &str) -> u32 {
    for idx in 0..pool.len() {
        if pool.get(idx as u32) == Some(value) {
            return idx as u32;
        }
    }
    0
}

fn build_pack_bytes(
    pack_kind: PackKind,
    id_map_hash: [u8; 32],
    locale_tag_sidx: u32,
    parent_tag_sidx: Option<u32>,
    build_epoch_ms: u64,
    sections: Vec<(u8, Vec<u8>)>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"MF2PACK\0");
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.push(match pack_kind {
        PackKind::Base => 0,
        PackKind::Overlay => 1,
        PackKind::IcuData => 2,
    });
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&id_map_hash);
    bytes.extend_from_slice(&locale_tag_sidx.to_le_bytes());
    let parent_raw = parent_tag_sidx.unwrap_or(u32::MAX);
    bytes.extend_from_slice(&parent_raw.to_le_bytes());
    bytes.extend_from_slice(&build_epoch_ms.to_le_bytes());
    bytes.extend_from_slice(&(sections.len() as u16).to_le_bytes());

    let directory_offset = bytes.len();
    let section_entry_len = 1 + 4 + 4;
    bytes.resize(directory_offset + sections.len() * section_entry_len, 0);

    let mut cursor = bytes.len();
    for (idx, (section_type, data)) in sections.into_iter().enumerate() {
        let offset = cursor as u32;
        let length = data.len() as u32;
        bytes.extend_from_slice(&data);
        let entry_offset = directory_offset + idx * section_entry_len;
        bytes[entry_offset] = section_type;
        bytes[entry_offset + 1..entry_offset + 5].copy_from_slice(&offset.to_le_bytes());
        bytes[entry_offset + 5..entry_offset + 9].copy_from_slice(&length.to_le_bytes());
        cursor = bytes.len();
    }

    bytes
}

struct StringInterner {
    map: BTreeMap<String, u32>,
    pool: StringPool,
}

impl StringInterner {
    fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            pool: StringPool::new(),
        }
    }

    fn intern(&mut self, value: &str) -> u32 {
        if let Some(idx) = self.map.get(value) {
            return *idx;
        }
        let idx = self.pool.push(value.to_string());
        let idx = idx as u32;
        self.map.insert(value.to_string(), idx);
        idx
    }

    fn into_pool(self) -> StringPool {
        self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_pack, PackBuildInput};
    use mf2_i18n_core::{BytecodeProgram, Catalog, MessageId, Opcode, PackCatalog, PackKind};
    use std::collections::BTreeMap;

    #[test]
    fn encodes_and_decodes_pack() {
        let mut program = BytecodeProgram::new();
        let sidx = program.string_pool.push("hello");
        program.opcodes.push(Opcode::EmitText { sidx });
        program.opcodes.push(Opcode::End);

        let mut messages = BTreeMap::new();
        messages.insert(MessageId::new(1), program);

        let bytes = encode_pack(&PackBuildInput {
            pack_kind: PackKind::Base,
            id_map_hash: [7u8; 32],
            locale_tag: "en".to_string(),
            parent_tag: None,
            build_epoch_ms: 0,
            messages,
        });

        let catalog = PackCatalog::decode(&bytes, &[7u8; 32]).expect("decode");
        let program = catalog.lookup(MessageId::new(1)).expect("program");
        assert_eq!(program.opcodes.len(), 2);
        let mut found = false;
        for idx in 0..program.string_pool.len() {
            if program.string_pool.get(idx as u32) == Some("hello") {
                found = true;
                break;
            }
        }
        assert!(found);
    }
}

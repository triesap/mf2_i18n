use alloc::string::String;
use alloc::vec::Vec;

use crate::{FormatterId, PluralCategory};

pub type StringIndex = u32;
pub type NumberIndex = u32;
pub type ArgIndex = u32;
pub type CaseTableIndex = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralRuleset {
    Cardinal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    EmitText { sidx: StringIndex },
    EmitStack,
    PushStr { sidx: StringIndex },
    PushNum { nidx: NumberIndex },
    PushArg { aidx: ArgIndex },
    Dup,
    Pop,
    CallFmt { fid: FormatterId, opt_count: u8 },
    Select { aidx: ArgIndex, table: CaseTableIndex },
    SelectPlural {
        aidx: ArgIndex,
        ruleset: PluralRuleset,
        table: CaseTableIndex,
    },
    Jump { rel: i32 },
    End,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseTable {
    pub entries: Vec<CaseEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseEntry {
    pub key: CaseKey,
    pub target: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaseKey {
    String(StringIndex),
    Exact(u32),
    Category(PluralCategory),
    Other,
}

pub struct StringPool {
    entries: Vec<String>,
}

impl StringPool {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn push(&mut self, value: impl Into<String>) -> StringIndex {
        let idx = self.entries.len();
        self.entries.push(value.into());
        idx as StringIndex
    }

    pub fn get(&self, index: StringIndex) -> Option<&str> {
        self.entries.get(index as usize).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for StringPool {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BytecodeProgram {
    pub opcodes: Vec<Opcode>,
    pub string_pool: StringPool,
    pub number_pool: Vec<f64>,
    pub case_tables: Vec<CaseTable>,
    pub arg_names: Vec<String>,
}

impl BytecodeProgram {
    pub fn new() -> Self {
        Self {
            opcodes: Vec::new(),
            string_pool: StringPool::new(),
            number_pool: Vec::new(),
            case_tables: Vec::new(),
            arg_names: Vec::new(),
        }
    }

    pub fn push_opcode(&mut self, opcode: Opcode) -> usize {
        self.opcodes.push(opcode);
        self.opcodes.len() - 1
    }

    pub fn push_arg_name(&mut self, name: impl Into<String>) -> ArgIndex {
        let idx = self.arg_names.len();
        self.arg_names.push(name.into());
        idx as ArgIndex
    }

    pub fn arg_name(&self, index: ArgIndex) -> Option<&str> {
        self.arg_names.get(index as usize).map(String::as_str)
    }
}

impl Default for BytecodeProgram {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::{BytecodeProgram, CaseEntry, CaseKey, CaseTable, Opcode, StringPool};

    #[test]
    fn string_pool_round_trips() {
        let mut pool = StringPool::new();
        let idx = pool.push("hello");
        assert_eq!(pool.get(idx), Some("hello"));
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn program_pushes_opcodes() {
        let mut program = BytecodeProgram::new();
        let pos = program.push_opcode(Opcode::EmitStack);
        assert_eq!(pos, 0);
        assert_eq!(program.opcodes.len(), 1);
    }

    #[test]
    fn case_table_stores_entries() {
        let table = CaseTable {
            entries: vec![CaseEntry {
                key: CaseKey::Other,
                target: 7,
            }],
        };
        assert_eq!(table.entries.len(), 1);
    }
}

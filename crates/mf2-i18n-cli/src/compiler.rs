use std::collections::BTreeMap;

use mf2_i18n_core::{
    BytecodeProgram, CaseEntry, CaseKey, CaseTable, FormatterId, Opcode, PluralRuleset,
};

use crate::parser::{CaseKey as AstCaseKey, Expr, Message, Segment, SelectKind, VarExpr};

pub struct CompileResult {
    pub program: BytecodeProgram,
}

pub fn compile_message(message: &Message) -> CompileResult {
    let mut compiler = Compiler::new();
    compiler.compile_message(message);
    compiler.program.opcodes.push(Opcode::End);
    CompileResult {
        program: compiler.program,
    }
}

struct Compiler {
    program: BytecodeProgram,
    arg_indices: BTreeMap<String, u32>,
}

impl Compiler {
    fn new() -> Self {
        Self {
            program: BytecodeProgram::new(),
            arg_indices: BTreeMap::new(),
        }
    }

    fn compile_message(&mut self, message: &Message) {
        for segment in &message.segments {
            match segment {
                Segment::Text { value, .. } => {
                    let sidx = self.program.string_pool.push(value.clone());
                    self.program.opcodes.push(Opcode::EmitText { sidx });
                }
                Segment::Expr(expr) => match expr {
                    Expr::Variable(var) => self.compile_var(var),
                    Expr::Select(select) => self.compile_select(select),
                },
            }
        }
    }

    fn compile_var(&mut self, var: &VarExpr) {
        let aidx = self.arg_index(&var.name);
        self.program.opcodes.push(Opcode::PushArg { aidx });
        if let Some(formatter) = &var.formatter {
            let fid = formatter_id(formatter);
            self.program
                .opcodes
                .push(Opcode::CallFmt { fid, opt_count: 0 });
        }
        self.program.opcodes.push(Opcode::EmitStack);
    }

    fn compile_select(&mut self, select: &crate::parser::SelectExpr) {
        let aidx = self.arg_index(&select.selector);
        let table_idx = self.program.case_tables.len() as u32;
        let select_pos = self.program.opcodes.len();
        let opcode = match select.kind {
            SelectKind::Plural => Opcode::SelectPlural {
                aidx,
                ruleset: PluralRuleset::Cardinal,
                table: table_idx,
            },
            SelectKind::Select => Opcode::Select {
                aidx,
                table: table_idx,
            },
        };
        self.program.opcodes.push(opcode);

        let mut entries = Vec::with_capacity(select.cases.len());
        let mut jumps = Vec::new();
        for case in &select.cases {
            let start = self.program.opcodes.len() as u32;
            entries.push(CaseEntry {
                key: compile_case_key(&mut self.program, &case.key, case.is_default),
                target: start,
            });
            self.compile_message(&case.value);
            let jump_pos = self.program.opcodes.len();
            self.program.opcodes.push(Opcode::Jump { rel: 0 });
            jumps.push(jump_pos);
        }

        let end = self.program.opcodes.len() as i32;
        for jump_pos in jumps {
            if let Opcode::Jump { rel } = &mut self.program.opcodes[jump_pos] {
                *rel = end - jump_pos as i32;
            }
        }

        if let Some(opcode) = self.program.opcodes.get_mut(select_pos) {
            *opcode = match select.kind {
                SelectKind::Plural => Opcode::SelectPlural {
                    aidx,
                    ruleset: PluralRuleset::Cardinal,
                    table: table_idx,
                },
                SelectKind::Select => Opcode::Select {
                    aidx,
                    table: table_idx,
                },
            };
        }

        self.program.case_tables.push(CaseTable { entries });
    }

    fn arg_index(&mut self, name: &str) -> u32 {
        if let Some(index) = self.arg_indices.get(name) {
            return *index;
        }
        let index = self.program.push_arg_name(name);
        self.arg_indices.insert(name.to_string(), index);
        index
    }
}

fn formatter_id(name: &str) -> FormatterId {
    match name {
        "number" => FormatterId::Number,
        "date" => FormatterId::Date,
        "time" => FormatterId::Time,
        "datetime" => FormatterId::DateTime,
        "unit" => FormatterId::Unit,
        "currency" => FormatterId::Currency,
        _ => FormatterId::Identity,
    }
}

fn compile_case_key(program: &mut BytecodeProgram, key: &AstCaseKey, is_default: bool) -> CaseKey {
    if is_default {
        return CaseKey::Other;
    }
    match key {
        AstCaseKey::Other => CaseKey::Other,
        AstCaseKey::Exact(value) => CaseKey::Exact(*value),
        AstCaseKey::Ident(value) => {
            let sidx = program.string_pool.push(value.clone());
            CaseKey::String(sidx)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_message;

    use super::compile_message;

    #[test]
    fn compiles_simple_message() {
        let message = parse_message("Hello { $name }").expect("parse");
        let compiled = compile_message(&message);
        assert!(!compiled.program.opcodes.is_empty());
    }

    #[test]
    fn compiles_select_message() {
        let message = parse_message("{ $count -> [one] {1} *[other] {n} }").expect("parse");
        let compiled = compile_message(&message);
        assert!(!compiled.program.case_tables.is_empty());
    }
}

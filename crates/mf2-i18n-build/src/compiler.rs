use std::collections::BTreeMap;

use mf2_i18n_core::{
    BytecodeProgram, CaseEntry, CaseKey, CaseTable, FormatterId, FormatterOption,
    FormatterOptionValue, Opcode, PluralRuleset,
};
use thiserror::Error;

use crate::parser::{
    CaseKey as AstCaseKey, Expr, FormatterOptionExpr, FormatterOptionExprValue, Message, Segment,
    SelectKind, VarExpr,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompileError {
    #[error("unknown formatter `{name}` at {line}:{column}")]
    UnknownFormatter {
        name: String,
        line: u32,
        column: u32,
    },
}

pub struct CompileResult {
    pub program: BytecodeProgram,
}

pub fn compile_message(message: &Message) -> Result<CompileResult, CompileError> {
    let mut compiler = Compiler::new();
    compiler.compile_message(message)?;
    compiler.program.opcodes.push(Opcode::End);
    Ok(CompileResult {
        program: compiler.program,
    })
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

    fn compile_message(&mut self, message: &Message) -> Result<(), CompileError> {
        for segment in &message.segments {
            match segment {
                Segment::Text { value, .. } => {
                    let sidx = self.program.string_pool.push(value.clone());
                    self.program.opcodes.push(Opcode::EmitText { sidx });
                }
                Segment::Expr(expr) => match expr {
                    Expr::Variable(var) => self.compile_var(var)?,
                    Expr::Select(select) => self.compile_select(select)?,
                },
            }
        }
        Ok(())
    }

    fn compile_var(&mut self, var: &VarExpr) -> Result<(), CompileError> {
        let aidx = self.arg_index(&var.name);
        self.program.opcodes.push(Opcode::PushArg { aidx });
        if let Some(formatter) = &var.formatter {
            let fid = formatter_id(&formatter.name, var.span.line, var.span.column)?;
            let opt_start = self.program.formatter_options.len() as u32;
            for option in &formatter.options {
                self.program
                    .push_formatter_option(compile_formatter_option(option));
            }
            let opt_count =
                u16::try_from(formatter.options.len()).expect("formatter option count exceeds u16");
            self.program.opcodes.push(Opcode::CallFmt {
                fid,
                opt_start,
                opt_count,
            });
        }
        self.program.opcodes.push(Opcode::EmitStack);
        Ok(())
    }

    fn compile_select(&mut self, select: &crate::parser::SelectExpr) -> Result<(), CompileError> {
        if let Some(formatter) = &select.formatter
            && formatter.name != "plural"
        {
            return Err(CompileError::UnknownFormatter {
                name: formatter.name.clone(),
                line: select.span.line,
                column: select.span.column,
            });
        }
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
            self.compile_message(&case.value)?;
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
        Ok(())
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

fn formatter_id(name: &str, line: u32, column: u32) -> Result<FormatterId, CompileError> {
    match name {
        "number" => Ok(FormatterId::Number),
        "date" => Ok(FormatterId::Date),
        "time" => Ok(FormatterId::Time),
        "datetime" => Ok(FormatterId::DateTime),
        "unit" => Ok(FormatterId::Unit),
        "currency" => Ok(FormatterId::Currency),
        "identity" => Ok(FormatterId::Identity),
        _ => Err(CompileError::UnknownFormatter {
            name: name.to_string(),
            line,
            column,
        }),
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

fn compile_formatter_option(option: &FormatterOptionExpr) -> FormatterOption {
    FormatterOption {
        key: option.key.clone(),
        value: match &option.value {
            FormatterOptionExprValue::Str(value) => FormatterOptionValue::Str(value.clone()),
            FormatterOptionExprValue::Num(value) => FormatterOptionValue::Num(*value),
            FormatterOptionExprValue::Bool(value) => FormatterOptionValue::Bool(*value),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_message;

    use super::{CompileError, compile_message};

    #[test]
    fn compiles_simple_message() {
        let message = parse_message("Hello { $name }").expect("parse");
        let compiled = compile_message(&message).expect("compile");
        assert!(!compiled.program.opcodes.is_empty());
    }

    #[test]
    fn compiles_select_message() {
        let message = parse_message("{ $count -> [one] {1} *[other] {n} }").expect("parse");
        let compiled = compile_message(&message).expect("compile");
        assert!(!compiled.program.case_tables.is_empty());
    }

    #[test]
    fn compiles_formatter_options_into_program() {
        let message = parse_message(
            "{ $value :number style=percent minimum-fraction-digits=2 use-grouping=true }",
        )
        .expect("parse");
        let compiled = compile_message(&message).expect("compile");
        assert_eq!(compiled.program.formatter_options.len(), 3);
        match compiled.program.opcodes[1] {
            mf2_i18n_core::Opcode::CallFmt {
                fid,
                opt_start,
                opt_count,
            } => {
                assert_eq!(fid, mf2_i18n_core::FormatterId::Number);
                assert_eq!(opt_start, 0);
                assert_eq!(opt_count, 3);
            }
            _ => panic!("expected call formatter"),
        }
    }

    #[test]
    fn rejects_unknown_variable_formatter() {
        let message = parse_message("{ $value :weird }").expect("parse");
        let err = match compile_message(&message) {
            Ok(_) => panic!("compile should fail"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            CompileError::UnknownFormatter {
                name: "weird".to_string(),
                line: 1,
                column: 3,
            }
        );
    }

    #[test]
    fn rejects_unknown_select_formatter() {
        let message = parse_message("{ $value :weird -> *[other] {x} }").expect("parse");
        let err = match compile_message(&message) {
            Ok(_) => panic!("compile should fail"),
            Err(err) => err,
        };
        assert_eq!(
            err,
            CompileError::UnknownFormatter {
                name: "weird".to_string(),
                line: 1,
                column: 3,
            }
        );
    }
}

use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    format_value, Args, BytecodeProgram, CaseKey, CaseTable, CoreError, CoreResult, FormatBackend,
    FormatterId, Opcode, PluralRuleset, Value,
};

pub fn execute(
    program: &BytecodeProgram,
    args: &Args,
    backend: &dyn FormatBackend,
) -> CoreResult<String> {
    let mut stack: Vec<Value> = Vec::new();
    let mut output = String::new();
    let mut pc: usize = 0;

    while pc < program.opcodes.len() {
        let opcode = program.opcodes[pc];
        match opcode {
            Opcode::EmitText { sidx } => {
                let text = program
                    .string_pool
                    .get(sidx)
                    .ok_or(CoreError::InvalidInput("string index out of bounds"))?;
                output.push_str(text);
            }
            Opcode::EmitStack => {
                let value = stack.pop().ok_or(CoreError::InvalidInput("stack underflow"))?;
                let rendered = format_value(backend, FormatterId::Identity, &value, &[])?;
                output.push_str(&rendered);
            }
            Opcode::PushStr { sidx } => {
                let text = program
                    .string_pool
                    .get(sidx)
                    .ok_or(CoreError::InvalidInput("string index out of bounds"))?;
                stack.push(Value::Str(String::from(text)));
            }
            Opcode::PushNum { nidx } => {
                let number = program
                    .number_pool
                    .get(nidx as usize)
                    .ok_or(CoreError::InvalidInput("number index out of bounds"))?;
                stack.push(Value::Num(*number));
            }
            Opcode::PushArg { aidx } => {
                let name = program
                    .arg_name(aidx)
                    .ok_or(CoreError::InvalidInput("arg index out of bounds"))?;
                let value = args.require(name)?;
                stack.push(clone_value(value)?);
            }
            Opcode::Dup => {
                let value = stack.last().ok_or(CoreError::InvalidInput("stack underflow"))?;
                stack.push(clone_value(value)?);
            }
            Opcode::Pop => {
                let _ = stack.pop().ok_or(CoreError::InvalidInput("stack underflow"))?;
            }
            Opcode::CallFmt { fid, opt_count } => {
                if opt_count != 0 {
                    return Err(CoreError::Unsupported("formatter options not supported"));
                }
                let value = stack.pop().ok_or(CoreError::InvalidInput("stack underflow"))?;
                let rendered = format_value(backend, fid, &value, &[])?;
                stack.push(Value::Str(rendered));
            }
            Opcode::Select { aidx, table } => {
                let target = select_case(program, args, aidx, table)?;
                pc = target;
                continue;
            }
            Opcode::SelectPlural {
                aidx,
                ruleset,
                table,
            } => {
                let target = select_plural_case(program, args, backend, aidx, ruleset, table)?;
                pc = target;
                continue;
            }
            Opcode::Jump { rel } => {
                let next = pc as i32 + rel;
                if next < 0 {
                    return Err(CoreError::InvalidInput("jump underflow"));
                }
                pc = next as usize;
                continue;
            }
            Opcode::End => break,
        }
        pc += 1;
    }

    Ok(output)
}

fn select_case(
    program: &BytecodeProgram,
    args: &Args,
    aidx: u32,
    table_idx: u32,
) -> CoreResult<usize> {
    let name = program
        .arg_name(aidx)
        .ok_or(CoreError::InvalidInput("arg index out of bounds"))?;
    let value = args.require(name)?;
    let value = match value {
        Value::Str(text) => text,
        _ => return Err(CoreError::InvalidInput("select expects string")),
    };
    let table = get_case_table(program, table_idx)?;
    match_case(table, program, value)
}

fn select_plural_case(
    program: &BytecodeProgram,
    args: &Args,
    backend: &dyn FormatBackend,
    aidx: u32,
    ruleset: PluralRuleset,
    table_idx: u32,
) -> CoreResult<usize> {
    let name = program
        .arg_name(aidx)
        .ok_or(CoreError::InvalidInput("arg index out of bounds"))?;
    let value = args.require(name)?;
    let number = match value {
        Value::Num(value) => *value,
        _ => return Err(CoreError::InvalidInput("plural expects number")),
    };
    let table = get_case_table(program, table_idx)?;
    if let Some(target) = match_exact_number(table, number) {
        return Ok(target);
    }
    if matches!(ruleset, PluralRuleset::Cardinal) {
        let category = backend.plural_category(number)?;
        if let Some(target) = match_plural_category(table, category) {
            return Ok(target);
        }
    }
    match_other(table)
}

fn get_case_table<'a>(
    program: &'a BytecodeProgram,
    table_idx: u32,
) -> CoreResult<&'a CaseTable> {
    program
        .case_tables
        .get(table_idx as usize)
        .ok_or(CoreError::InvalidInput("case table index out of bounds"))
}

fn match_case(table: &CaseTable, program: &BytecodeProgram, value: &str) -> CoreResult<usize> {
    let mut other = None;
    for entry in &table.entries {
        match &entry.key {
            CaseKey::String(sidx) => {
                if let Some(candidate) = program.string_pool.get(*sidx) {
                    if candidate == value {
                        return Ok(entry.target as usize);
                    }
                }
            }
            CaseKey::Other => other = Some(entry.target as usize),
            _ => {}
        }
    }
    other.ok_or(CoreError::InvalidInput("missing other case"))
}

fn match_exact_number(table: &CaseTable, value: f64) -> Option<usize> {
    if value < 0.0 {
        return None;
    }
    let candidate = value as u32;
    if (candidate as f64) != value {
        return None;
    }
    for entry in &table.entries {
        if let CaseKey::Exact(exact) = entry.key {
            if exact == candidate {
                return Some(entry.target as usize);
            }
        }
    }
    None
}

fn match_plural_category(table: &CaseTable, category: crate::PluralCategory) -> Option<usize> {
    for entry in &table.entries {
        if let CaseKey::Category(case_category) = entry.key {
            if case_category == category {
                return Some(entry.target as usize);
            }
        }
    }
    None
}

fn match_other(table: &CaseTable) -> CoreResult<usize> {
    table
        .entries
        .iter()
        .find_map(|entry| match entry.key {
            CaseKey::Other => Some(entry.target as usize),
            _ => None,
        })
        .ok_or(CoreError::InvalidInput("missing other case"))
}

fn clone_value(value: &Value) -> CoreResult<Value> {
    match value {
        Value::Str(text) => Ok(Value::Str(text.clone())),
        Value::Num(number) => Ok(Value::Num(*number)),
        Value::Bool(value) => Ok(Value::Bool(*value)),
        Value::DateTime(value) => Ok(Value::DateTime(*value)),
        Value::Unit { value, unit_id } => Ok(Value::Unit {
            value: *value,
            unit_id: *unit_id,
        }),
        Value::Currency { value, code } => Ok(Value::Currency {
            value: *value,
            code: *code,
        }),
        Value::Any(_) => Err(CoreError::Unsupported("cloning any value")),
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;

    use super::execute;
    use crate::{
        Args, BytecodeProgram, FormatBackend, FormatterId, FormatterOption, Opcode, PluralCategory,
        Value,
    };

    struct TestBackend;

    impl FormatBackend for TestBackend {
        fn plural_category(&self, _value: f64) -> crate::CoreResult<PluralCategory> {
            Ok(PluralCategory::Other)
        }

        fn format_number(&self, value: f64, _options: &[FormatterOption]) -> crate::CoreResult<String> {
            Ok(format!("num:{value}"))
        }

        fn format_date(&self, value: i64, _options: &[FormatterOption]) -> crate::CoreResult<String> {
            Ok(format!("date:{value}"))
        }

        fn format_time(&self, value: i64, _options: &[FormatterOption]) -> crate::CoreResult<String> {
            Ok(format!("time:{value}"))
        }

        fn format_datetime(&self, value: i64, _options: &[FormatterOption]) -> crate::CoreResult<String> {
            Ok(format!("datetime:{value}"))
        }

        fn format_unit(
            &self,
            value: f64,
            unit_id: u32,
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
            Ok(format!("unit:{value}:{unit_id}"))
        }

        fn format_currency(
            &self,
            value: f64,
            code: [u8; 3],
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
            let code = core::str::from_utf8(&code).unwrap_or("???");
            Ok(format!("currency:{value}:{code}"))
        }
    }

    #[test]
    fn executes_emit_text_and_stack() {
        let backend = TestBackend;
        let mut program = BytecodeProgram::new();
        let hello = program.string_pool.push("Hello ");
        let name_arg = program.push_arg_name("name");
        program.opcodes = vec![
            Opcode::EmitText { sidx: hello },
            Opcode::PushArg { aidx: name_arg },
            Opcode::EmitStack,
            Opcode::End,
        ];

        let mut args = Args::new();
        args.insert("name", Value::Str(String::from("Nova")));

        let out = execute(&program, &args, &backend).expect("exec ok");
        assert_eq!(out, "Hello Nova");
    }

    #[test]
    fn executes_call_fmt() {
        let backend = TestBackend;
        let mut program = BytecodeProgram::new();
        program.number_pool.push(3.5);
        program.opcodes = vec![
            Opcode::PushNum { nidx: 0 },
            Opcode::CallFmt {
                fid: FormatterId::Number,
                opt_count: 0,
            },
            Opcode::EmitStack,
            Opcode::End,
        ];

        let args = Args::new();
        let out = execute(&program, &args, &backend).expect("exec ok");
        assert_eq!(out, "num:3.5");
    }

    #[test]
    fn executes_select_branch() {
        let backend = TestBackend;
        let mut program = BytecodeProgram::new();
        let key_arg = program.push_arg_name("key");
        let key_idx = program.string_pool.push("x");
        let foo_idx = program.string_pool.push("foo");
        let bar_idx = program.string_pool.push("bar");
        program.case_tables.push(crate::CaseTable {
            entries: vec![
                crate::CaseEntry {
                    key: crate::CaseKey::String(key_idx),
                    target: 1,
                },
                crate::CaseEntry {
                    key: crate::CaseKey::Other,
                    target: 3,
                },
            ],
        });
        program.opcodes = vec![
            Opcode::Select {
                aidx: key_arg,
                table: 0,
            },
            Opcode::EmitText { sidx: foo_idx },
            Opcode::Jump { rel: 2 },
            Opcode::EmitText { sidx: bar_idx },
            Opcode::End,
        ];

        let mut args = Args::new();
        args.insert("key", Value::Str(String::from("x")));
        let out = execute(&program, &args, &backend).expect("exec ok");
        assert_eq!(out, "foo");
    }

    #[test]
    fn executes_plural_branch() {
        let backend = TestBackend;
        let mut program = BytecodeProgram::new();
        let count_arg = program.push_arg_name("count");
        let one_idx = program.string_pool.push("one");
        let other_idx = program.string_pool.push("other");
        program.case_tables.push(crate::CaseTable {
            entries: vec![
                crate::CaseEntry {
                    key: crate::CaseKey::Exact(1),
                    target: 1,
                },
                crate::CaseEntry {
                    key: crate::CaseKey::Other,
                    target: 3,
                },
            ],
        });
        program.opcodes = vec![
            Opcode::SelectPlural {
                aidx: count_arg,
                ruleset: crate::PluralRuleset::Cardinal,
                table: 0,
            },
            Opcode::EmitText { sidx: one_idx },
            Opcode::Jump { rel: 2 },
            Opcode::EmitText { sidx: other_idx },
            Opcode::End,
        ];

        let mut args = Args::new();
        args.insert("count", Value::Num(2.0));
        let out = execute(&program, &args, &backend).expect("exec ok");
        assert_eq!(out, "other");
    }
}

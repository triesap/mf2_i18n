use alloc::string::String;
use alloc::vec::Vec;

use crate::{
    format_value, Args, BytecodeProgram, CoreError, CoreResult, FormatBackend, FormatterId, Opcode,
    Value,
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
        match program.opcodes[pc] {
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
            Opcode::Select { .. } | Opcode::SelectPlural { .. } | Opcode::Jump { .. } => {
                return Err(CoreError::Unsupported("control flow not supported"));
            }
            Opcode::End => break,
        }
        pc += 1;
    }

    Ok(output)
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
}

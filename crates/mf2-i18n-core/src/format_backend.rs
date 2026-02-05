use alloc::format;
use alloc::string::{String, ToString};

use crate::{CoreError, CoreResult, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormatterId {
    Number,
    Date,
    Time,
    DateTime,
    Unit,
    Currency,
    Identity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormatterOptionValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FormatterOption {
    pub key: String,
    pub value: FormatterOptionValue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

pub trait FormatBackend {
    fn plural_category(&self, value: f64) -> CoreResult<PluralCategory>;
    fn format_number(&self, value: f64, options: &[FormatterOption]) -> CoreResult<String>;
    fn format_date(&self, value: i64, options: &[FormatterOption]) -> CoreResult<String>;
    fn format_time(&self, value: i64, options: &[FormatterOption]) -> CoreResult<String>;
    fn format_datetime(&self, value: i64, options: &[FormatterOption]) -> CoreResult<String>;
    fn format_unit(
        &self,
        value: f64,
        unit_id: u32,
        options: &[FormatterOption],
    ) -> CoreResult<String>;
    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        options: &[FormatterOption],
    ) -> CoreResult<String>;
}

pub fn format_value(
    backend: &dyn FormatBackend,
    formatter: FormatterId,
    value: &Value,
    options: &[FormatterOption],
) -> CoreResult<String> {
    match formatter {
        FormatterId::Number => match value {
            Value::Num(number) => backend.format_number(*number, options),
            _ => Err(CoreError::InvalidInput("formatter expects number")),
        },
        FormatterId::Date => match value {
            Value::DateTime(timestamp) => backend.format_date(*timestamp, options),
            _ => Err(CoreError::InvalidInput("formatter expects datetime")),
        },
        FormatterId::Time => match value {
            Value::DateTime(timestamp) => backend.format_time(*timestamp, options),
            _ => Err(CoreError::InvalidInput("formatter expects datetime")),
        },
        FormatterId::DateTime => match value {
            Value::DateTime(timestamp) => backend.format_datetime(*timestamp, options),
            _ => Err(CoreError::InvalidInput("formatter expects datetime")),
        },
        FormatterId::Unit => match value {
            Value::Unit { value, unit_id } => backend.format_unit(*value, *unit_id, options),
            _ => Err(CoreError::InvalidInput("formatter expects unit")),
        },
        FormatterId::Currency => match value {
            Value::Currency { value, code } => backend.format_currency(*value, *code, options),
            _ => Err(CoreError::InvalidInput("formatter expects currency")),
        },
        FormatterId::Identity => format_value_default(value),
    }
}

fn format_value_default(value: &Value) -> CoreResult<String> {
    match value {
        Value::Str(text) => Ok(text.clone()),
        Value::Num(number) => Ok(number.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::DateTime(timestamp) => Ok(timestamp.to_string()),
        Value::Unit { value, unit_id } => Ok(format!("{value}:{unit_id}")),
        Value::Currency { value, code } => {
            let code =
                core::str::from_utf8(code).map_err(|_| CoreError::InvalidInput("currency code"))?;
            Ok(format!("{value}:{code}"))
        }
        Value::Any(_) => Err(CoreError::Unsupported("identity formatting for any value")),
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;

    use super::{FormatBackend, FormatterId, FormatterOption, PluralCategory, format_value};
    use crate::Value;

    struct TestBackend;

    impl FormatBackend for TestBackend {
        fn plural_category(&self, _value: f64) -> crate::CoreResult<PluralCategory> {
            Ok(PluralCategory::Other)
        }

        fn format_number(
            &self,
            value: f64,
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
            Ok(format!("num:{value}"))
        }

        fn format_date(
            &self,
            value: i64,
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
            Ok(format!("date:{value}"))
        }

        fn format_time(
            &self,
            value: i64,
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
            Ok(format!("time:{value}"))
        }

        fn format_datetime(
            &self,
            value: i64,
            _options: &[FormatterOption],
        ) -> crate::CoreResult<String> {
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
    fn format_value_dispatches() {
        let backend = TestBackend;
        let options = [];
        let value = Value::Num(3.5);
        let out = format_value(&backend, FormatterId::Number, &value, &options).expect("format ok");
        assert_eq!(out, "num:3.5");
    }

    #[test]
    fn identity_formats_string() {
        let backend = TestBackend;
        let options = [];
        let value = Value::Str(String::from("hello"));
        let out =
            format_value(&backend, FormatterId::Identity, &value, &options).expect("format ok");
        assert_eq!(out, "hello");
    }
}

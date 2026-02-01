use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::{CoreError, CoreResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArgType {
    Str,
    Num,
    Bool,
    DateTime,
    Unit,
    Currency,
    Any,
}

impl ArgType {
    pub fn matches(self, value: &Value) -> bool {
        match (self, value) {
            (ArgType::Any, _) => true,
            (ArgType::Str, Value::Str(_)) => true,
            (ArgType::Num, Value::Num(_)) => true,
            (ArgType::Bool, Value::Bool(_)) => true,
            (ArgType::DateTime, Value::DateTime(_)) => true,
            (ArgType::Unit, Value::Unit { .. }) => true,
            (ArgType::Currency, Value::Currency { .. }) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    DateTime(i64),
    Unit { value: f64, unit_id: u32 },
    Currency { value: f64, code: [u8; 3] },
    Any(Box<dyn core::any::Any>),
}

pub struct Args {
    values: BTreeMap<String, Value>,
}

impl Args {
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, value: Value) -> Option<Value> {
        self.values.insert(name.into(), value)
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    pub fn require(&self, name: &str) -> CoreResult<&Value> {
        self.values
            .get(name)
            .ok_or(CoreError::InvalidInput("missing argument"))
    }

    pub fn validate_type(&self, name: &str, expected: ArgType) -> CoreResult<()> {
        let value = self.require(name)?;
        if expected.matches(value) {
            Ok(())
        } else {
            Err(CoreError::InvalidInput("argument type mismatch"))
        }
    }
}

impl Default for Args {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::{ArgType, Args, Value};

    #[test]
    fn args_insert_and_get() {
        let mut args = Args::new();
        args.insert("name", Value::Str(String::from("Nova")));
        let value = args.get("name").expect("value should exist");
        match value {
            Value::Str(value) => assert_eq!(value, "Nova"),
            _ => panic!("unexpected value type"),
        }
    }

    #[test]
    fn require_reports_missing_argument() {
        let args = Args::new();
        let err = args.require("missing").expect_err("missing should error");
        assert_eq!(err, crate::CoreError::InvalidInput("missing argument"));
    }

    #[test]
    fn validate_type_accepts_expected() {
        let mut args = Args::new();
        args.insert("count", Value::Num(42.0));
        let result = args.validate_type("count", ArgType::Num);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_type_rejects_mismatch() {
        let mut args = Args::new();
        args.insert("count", Value::Num(42.0));
        let err = args
            .validate_type("count", ArgType::Str)
            .expect_err("type mismatch should error");
        assert_eq!(err, crate::CoreError::InvalidInput("argument type mismatch"));
    }
}

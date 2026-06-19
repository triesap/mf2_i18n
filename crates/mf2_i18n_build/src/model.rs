use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
#[serde(rename_all = "lowercase")]
pub enum ArgType {
    String,
    Number,
    Bool,
    DateTime,
    Unit,
    Currency,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
pub struct ArgSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
pub struct MessageSpec {
    pub key: String,
    pub args: Vec<ArgSpec>,
}

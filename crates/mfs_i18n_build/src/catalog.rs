use serde::{Deserialize, Serialize};

use crate::model::ArgSpec;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub schema: u32,
    pub project: String,
    pub generated_at: String,
    pub default_locale: String,
    pub messages: Vec<CatalogMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogMessage {
    pub key: String,
    pub id: u32,
    pub args: Vec<ArgSpec>,
    pub features: CatalogFeatures,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_refs: Option<Vec<SourceRef>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogFeatures {
    pub select: bool,
    pub plural_cardinal: bool,
    pub plural_ordinal: bool,
    pub formatters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use crate::model::{ArgSpec, ArgType};

    use super::{Catalog, CatalogFeatures, CatalogMessage};

    #[test]
    fn serializes_catalog_schema() {
        let catalog = Catalog {
            schema: 1,
            project: "demo".to_string(),
            generated_at: "2026-02-01T00:00:00Z".to_string(),
            default_locale: "en".to_string(),
            messages: vec![CatalogMessage {
                key: "home.title".to_string(),
                id: 42,
                args: vec![ArgSpec {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                }],
                features: CatalogFeatures::default(),
                source_refs: None,
            }],
        };

        let value = serde_json::to_value(&catalog).expect("json");
        assert_eq!(value["schema"], Value::from(1));
        assert_eq!(value["project"], Value::from("demo"));
        assert_eq!(value["messages"][0]["key"], Value::from("home.title"));
    }
}

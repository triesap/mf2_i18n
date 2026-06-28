use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

use crate::model::ArgType;
use crate::parser::{Expr, Message, Segment, SelectExpr, SelectKind, VarExpr, parse_message};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogMessage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
pub struct MessageMetadata {
    pub id: String,
    pub namespace: String,
    pub message_path: String,
    pub source_path: String,
    pub source_line: Option<u32>,
    pub args: Vec<MessageArg>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "dto-bindgen", derive(dto_bindgen::Dto))]
pub struct MessageArg {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    pub required: bool,
}

#[derive(Debug, Error)]
pub enum MessageMetadataError {
    #[error(
        "failed to parse i18n message metadata for locale {locale} key {key}: {message} at {line}:{column}"
    )]
    Parse {
        locale: String,
        key: String,
        message: String,
        line: u32,
        column: u32,
    },
}

pub fn collect_message_metadata(
    locale: &str,
    catalog: &ProjectCatalog,
    namespace_filter: Option<&[String]>,
) -> Result<Vec<MessageMetadata>, MessageMetadataError> {
    let mut metadata = Vec::new();
    for message in catalog.values() {
        if namespace_filter.is_some_and(|values| !values.contains(&message.namespace)) {
            continue;
        }
        metadata.push(metadata_for_message(locale, message)?);
    }
    Ok(metadata)
}

pub fn infer_message_args(message: &Message) -> Vec<MessageArg> {
    let mut args = BTreeMap::<String, ArgType>::new();
    infer_segments(&message.segments, &mut args);
    args.into_iter()
        .map(|(name, arg_type)| MessageArg {
            name,
            arg_type,
            required: true,
        })
        .collect()
}

fn metadata_for_message(
    locale: &str,
    message: &ProjectCatalogMessage,
) -> Result<MessageMetadata, MessageMetadataError> {
    let parsed = parse_message(&message.value).map_err(|error| MessageMetadataError::Parse {
        locale: locale.to_owned(),
        key: message.qualified_key.clone(),
        message: error.message,
        line: error.span.line,
        column: error.span.column,
    })?;

    Ok(MessageMetadata {
        id: message.qualified_key.clone(),
        namespace: message.namespace.clone(),
        message_path: message.message_path.clone(),
        source_path: message.source_path.to_string_lossy().into_owned(),
        source_line: message.line,
        args: infer_message_args(&parsed),
    })
}

fn infer_segments(segments: &[Segment], args: &mut BTreeMap<String, ArgType>) {
    for segment in segments {
        match segment {
            Segment::Text { .. } => {}
            Segment::Expr(Expr::Variable(var)) => infer_variable(var, args),
            Segment::Expr(Expr::Select(select)) => infer_select(select, args),
        }
    }
}

fn infer_variable(var: &VarExpr, args: &mut BTreeMap<String, ArgType>) {
    let arg_type = var
        .formatter
        .as_ref()
        .map(|formatter| formatter_arg_type(&formatter.name))
        .unwrap_or(ArgType::String);
    merge_arg(args, &var.name, arg_type);
}

fn infer_select(select: &SelectExpr, args: &mut BTreeMap<String, ArgType>) {
    let arg_type = match select.kind {
        SelectKind::Select => ArgType::String,
        SelectKind::Plural => ArgType::Number,
    };
    merge_arg(args, &select.selector, arg_type);

    for case in &select.cases {
        infer_segments(&case.value.segments, args);
    }
}

fn merge_arg(args: &mut BTreeMap<String, ArgType>, name: &str, arg_type: ArgType) {
    args.entry(name.to_owned())
        .and_modify(|existing| {
            if *existing != arg_type {
                *existing = ArgType::Any;
            }
        })
        .or_insert(arg_type);
}

fn formatter_arg_type(formatter: &str) -> ArgType {
    match formatter {
        "number" | "plural" => ArgType::Number,
        "date" | "time" | "datetime" => ArgType::DateTime,
        "unit" => ArgType::Unit,
        "currency" => ArgType::Currency,
        "identity" => ArgType::Any,
        _ => ArgType::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_message_metadata, infer_message_args};
    use crate::model::ArgType;
    use crate::parser::parse_message;
    use crate::project_catalogs::{ProjectCatalog, ProjectCatalogMessage};
    use std::path::PathBuf;

    #[test]
    fn infers_arguments_from_variables_formatters_and_selects() {
        let message = parse_message(
            "Hello { $name }. { $count :plural -> [one] {{ $count :number } item} *[other] {{ $count :number } items} } on { $when :datetime }",
        )
        .expect("parse");

        let args = infer_message_args(&message);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].arg_type, ArgType::Number);
        assert_eq!(args[1].name, "name");
        assert_eq!(args[1].arg_type, ArgType::String);
        assert_eq!(args[2].name, "when");
        assert_eq!(args[2].arg_type, ArgType::DateTime);
    }

    #[test]
    fn collects_source_metadata_from_default_catalog() {
        let mut catalog = ProjectCatalog::new();
        catalog.insert(
            "common.home.title".to_string(),
            ProjectCatalogMessage {
                namespace: "common".to_string(),
                message_path: "home.title".to_string(),
                qualified_key: "common.home.title".to_string(),
                value: "Hi { $name }".to_string(),
                source_path: PathBuf::from("locales/en/common.mf2"),
                line: Some(7),
            },
        );

        let metadata = collect_message_metadata("en", &catalog, None).expect("metadata");

        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].id, "common.home.title");
        assert_eq!(metadata[0].namespace, "common");
        assert_eq!(metadata[0].message_path, "home.title");
        assert_eq!(metadata[0].source_path, "locales/en/common.mf2");
        assert_eq!(metadata[0].source_line, Some(7));
        assert_eq!(metadata[0].args[0].name, "name");
    }

    #[test]
    fn conflicting_inferences_become_any() {
        let message = parse_message("{ $value } { $value :number }").expect("parse");
        let args = infer_message_args(&message);

        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "value");
        assert_eq!(args[0].arg_type, ArgType::Any);
    }
}

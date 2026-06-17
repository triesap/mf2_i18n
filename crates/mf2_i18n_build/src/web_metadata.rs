use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::model::ArgType;
use crate::parser::{Expr, Message, Segment, SelectExpr, SelectKind, VarExpr, parse_message};
use crate::project_catalogs::{ProjectCatalog, ProjectCatalogMessage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebMessageMetadata {
    pub id: String,
    pub namespace: String,
    pub message_path: String,
    pub args: Vec<WebMessageArg>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HybridMessageDelivery {
    StaticJson,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridMessageRoute {
    pub id: String,
    pub namespace: String,
    pub message_path: String,
    pub delivery: HybridMessageDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridMessagePlan {
    pub static_messages: Vec<HybridMessageRoute>,
    pub runtime_messages: Vec<HybridMessageRoute>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebMessageArg {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ArgType,
    pub required: bool,
}

#[derive(Debug, Error)]
pub enum WebMessageMetadataError {
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

pub fn collect_web_message_metadata(
    locale: &str,
    catalog: &ProjectCatalog,
    namespace_filter: Option<&[String]>,
) -> Result<Vec<WebMessageMetadata>, WebMessageMetadataError> {
    let mut metadata = Vec::new();
    for message in catalog.values() {
        if namespace_filter.is_some_and(|values| !values.contains(&message.namespace)) {
            continue;
        }
        metadata.push(metadata_for_message(locale, message)?);
    }
    Ok(metadata)
}

pub fn infer_web_message_args(message: &Message) -> Vec<WebMessageArg> {
    let mut args = BTreeMap::<String, ArgType>::new();
    infer_segments(&message.segments, &mut args);
    args.into_iter()
        .map(|(name, arg_type)| WebMessageArg {
            name,
            arg_type,
            required: true,
        })
        .collect()
}

pub fn classify_hybrid_messages(
    locale: &str,
    catalog: &ProjectCatalog,
    namespace_filter: Option<&[String]>,
) -> Result<HybridMessagePlan, WebMessageMetadataError> {
    let mut static_messages = Vec::new();
    let mut runtime_messages = Vec::new();

    for message in catalog.values() {
        if namespace_filter.is_some_and(|values| !values.contains(&message.namespace)) {
            continue;
        }
        let parsed =
            parse_message(&message.value).map_err(|error| WebMessageMetadataError::Parse {
                locale: locale.to_owned(),
                key: message.qualified_key.clone(),
                message: error.message,
                line: error.span.line,
                column: error.span.column,
            })?;
        let delivery = hybrid_delivery_for_message(&parsed);
        let route = HybridMessageRoute {
            id: message.qualified_key.clone(),
            namespace: message.namespace.clone(),
            message_path: message.message_path.clone(),
            delivery,
        };
        match delivery {
            HybridMessageDelivery::StaticJson => static_messages.push(route),
            HybridMessageDelivery::Runtime => runtime_messages.push(route),
        }
    }

    Ok(HybridMessagePlan {
        static_messages,
        runtime_messages,
    })
}

pub fn render_web_typescript_metadata_types(
    default_locale: &str,
    supported_locales: &[String],
    namespaces: &[String],
    message_metadata: &[WebMessageMetadata],
) -> String {
    let message_ids = if message_metadata.is_empty() {
        "never".to_string()
    } else {
        message_metadata
            .iter()
            .map(|message| format!("  | {}", json_string(&message.id)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let message_args = if message_metadata.is_empty() {
        "Record<never, never>".to_string()
    } else {
        let entries = message_metadata
            .iter()
            .map(|message| {
                format!(
                    "  {}: {};",
                    json_string(&message.id),
                    render_message_args_type(&message.args)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{{\n{entries}\n}}")
    };

    format!(
        "export const DEFAULT_LOCALE = {} as const;\nexport const SUPPORTED_LOCALES = [{}] as const;\nexport type Locale = typeof SUPPORTED_LOCALES[number];\nexport const MESSAGE_NAMESPACES = [{}] as const;\nexport type Namespace = typeof MESSAGE_NAMESPACES[number];\nexport type MessageId =\n{};\nexport type MessageArgsById = {};\nexport type ArgsFor<TMessageId extends MessageId> = MessageArgsById[TMessageId];\n",
        json_string(default_locale),
        render_string_array(supported_locales),
        render_string_array(namespaces),
        message_ids,
        message_args,
    )
}

fn metadata_for_message(
    locale: &str,
    message: &ProjectCatalogMessage,
) -> Result<WebMessageMetadata, WebMessageMetadataError> {
    let parsed = parse_message(&message.value).map_err(|error| WebMessageMetadataError::Parse {
        locale: locale.to_owned(),
        key: message.qualified_key.clone(),
        message: error.message,
        line: error.span.line,
        column: error.span.column,
    })?;

    Ok(WebMessageMetadata {
        id: message.qualified_key.clone(),
        namespace: message.namespace.clone(),
        message_path: message.message_path.clone(),
        args: infer_web_message_args(&parsed),
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

fn hybrid_delivery_for_message(message: &Message) -> HybridMessageDelivery {
    if message
        .segments
        .iter()
        .all(|segment| matches!(segment, Segment::Text { .. }))
    {
        HybridMessageDelivery::StaticJson
    } else {
        HybridMessageDelivery::Runtime
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

fn render_message_args_type(args: &[WebMessageArg]) -> String {
    if args.is_empty() {
        "undefined".to_string()
    } else {
        let entries = args
            .iter()
            .map(|arg| {
                format!(
                    "    readonly {}: {};",
                    json_string(&arg.name),
                    render_arg_type(&arg.arg_type)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{{\n{entries}\n  }}")
    }
}

fn render_arg_type(arg_type: &ArgType) -> &'static str {
    match arg_type {
        ArgType::String => "string",
        ArgType::Number => "number",
        ArgType::Bool => "boolean",
        ArgType::DateTime => "Date | string | number",
        ArgType::Unit => "{ readonly value: number; readonly unit: string }",
        ArgType::Currency => "{ readonly value: number; readonly currency: string }",
        ArgType::Any => "unknown",
    }
}

fn render_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string serialization should not fail")
}

#[cfg(test)]
mod tests {
    use super::{
        HybridMessageDelivery, classify_hybrid_messages, collect_web_message_metadata,
        infer_web_message_args,
    };
    use crate::model::ArgType;
    use crate::parser::parse_message;
    use crate::project_catalogs::{ProjectCatalog, ProjectCatalogMessage};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn infers_arguments_from_variables_formatters_and_selects() {
        let message = parse_message(
            "Hello { $name }. { $count :plural -> [one] {{ $count :number } item} *[other] {{ $count :number } items} } on { $when :datetime }",
        )
        .expect("parse");

        let args = infer_web_message_args(&message);

        assert_eq!(args.len(), 3);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].arg_type, ArgType::Number);
        assert_eq!(args[1].name, "name");
        assert_eq!(args[1].arg_type, ArgType::String);
        assert_eq!(args[2].name, "when");
        assert_eq!(args[2].arg_type, ArgType::DateTime);
    }

    #[test]
    fn collects_metadata_from_default_catalog() {
        let mut catalog = ProjectCatalog::new();
        catalog.insert(
            "common.home.title".to_string(),
            ProjectCatalogMessage {
                namespace: "common".to_string(),
                message_path: "home.title".to_string(),
                qualified_key: "common.home.title".to_string(),
                value: "Hi { $name }".to_string(),
                source_path: PathBuf::from("locales/en/common.mf2"),
                line: Some(1),
            },
        );
        catalog.insert(
            "docs.hidden".to_string(),
            ProjectCatalogMessage {
                namespace: "docs".to_string(),
                message_path: "hidden".to_string(),
                qualified_key: "docs.hidden".to_string(),
                value: "Hidden".to_string(),
                source_path: PathBuf::from("locales/en/docs.mf2"),
                line: Some(1),
            },
        );

        let metadata = collect_web_message_metadata("en", &catalog, Some(&["common".to_string()]))
            .expect("metadata");

        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].id, "common.home.title");
        assert_eq!(metadata[0].args[0].name, "name");
    }

    #[test]
    fn conflicting_inferences_become_any() {
        let message = parse_message("{ $value } { $value :number }").expect("parse");
        let args = infer_web_message_args(&message);

        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "value");
        assert_eq!(args[0].arg_type, ArgType::Any);
    }

    #[test]
    fn metadata_parse_errors_include_locale_and_key() {
        let mut catalog = BTreeMap::new();
        catalog.insert(
            "common.bad".to_string(),
            ProjectCatalogMessage {
                namespace: "common".to_string(),
                message_path: "bad".to_string(),
                qualified_key: "common.bad".to_string(),
                value: "{ $name ".to_string(),
                source_path: PathBuf::from("locales/en/common.mf2"),
                line: Some(1),
            },
        );

        let err = collect_web_message_metadata("en", &catalog, None).expect_err("parse error");
        assert!(err.to_string().contains("locale en key common.bad"));
    }

    #[test]
    fn renders_typescript_metadata_contract() {
        let metadata = vec![super::WebMessageMetadata {
            id: "common.count".to_string(),
            namespace: "common".to_string(),
            message_path: "count".to_string(),
            args: vec![super::WebMessageArg {
                name: "count".to_string(),
                arg_type: ArgType::Number,
                required: true,
            }],
        }];

        let types = super::render_web_typescript_metadata_types(
            "en",
            &["en".to_string(), "es".to_string()],
            &["common".to_string()],
            &metadata,
        );

        assert!(types.contains("export type MessageId"));
        assert!(types.contains("\"common.count\""));
        assert!(types.contains("readonly \"count\": number"));
    }

    #[test]
    fn classifies_hybrid_static_and_runtime_messages() {
        let mut catalog = ProjectCatalog::new();
        catalog.insert(
            "common.title".to_string(),
            ProjectCatalogMessage {
                namespace: "common".to_string(),
                message_path: "title".to_string(),
                qualified_key: "common.title".to_string(),
                value: "Welcome".to_string(),
                source_path: PathBuf::from("locales/en/common.mf2"),
                line: Some(1),
            },
        );
        catalog.insert(
            "common.count".to_string(),
            ProjectCatalogMessage {
                namespace: "common".to_string(),
                message_path: "count".to_string(),
                qualified_key: "common.count".to_string(),
                value: "{ $count :plural -> [one] {One item} *[other] {{ $count :number } items} }"
                    .to_string(),
                source_path: PathBuf::from("locales/en/common.mf2"),
                line: Some(2),
            },
        );
        catalog.insert(
            "docs.hidden".to_string(),
            ProjectCatalogMessage {
                namespace: "docs".to_string(),
                message_path: "hidden".to_string(),
                qualified_key: "docs.hidden".to_string(),
                value: "Hidden".to_string(),
                source_path: PathBuf::from("locales/en/docs.mf2"),
                line: Some(1),
            },
        );

        let plan =
            classify_hybrid_messages("en", &catalog, Some(&["common".to_string()])).expect("plan");

        assert_eq!(plan.static_messages.len(), 1);
        assert_eq!(plan.static_messages[0].id, "common.title");
        assert_eq!(
            plan.static_messages[0].delivery,
            HybridMessageDelivery::StaticJson
        );
        assert_eq!(plan.runtime_messages.len(), 1);
        assert_eq!(plan.runtime_messages[0].id, "common.count");
        assert_eq!(
            plan.runtime_messages[0].delivery,
            HybridMessageDelivery::Runtime
        );
    }
}

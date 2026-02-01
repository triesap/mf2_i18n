use crate::diagnostic::Diagnostic;
use crate::model::{ArgType, MessageSpec};
use crate::parser::{CaseKey, Expr, Message, Segment, SelectExpr, SelectKind, VarExpr};

pub fn validate_message(message: &Message, spec: &MessageSpec) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    validate_segments(&message.segments, spec, &mut diagnostics);
    diagnostics
}

fn validate_segments(segments: &[Segment], spec: &MessageSpec, diagnostics: &mut Vec<Diagnostic>) {
    for segment in segments {
        match segment {
            Segment::Text { .. } => {}
            Segment::Expr(expr) => match expr {
                Expr::Variable(var) => validate_var(var, spec, diagnostics),
                Expr::Select(select) => validate_select(select, spec, diagnostics),
            },
        }
    }
}

fn validate_var(var: &VarExpr, spec: &MessageSpec, diagnostics: &mut Vec<Diagnostic>) {
    if let Some(arg) = spec.args.iter().find(|arg| arg.name == var.name) {
        if let Some(formatter) = &var.formatter {
            if !is_known_formatter(formatter) {
                diagnostics.push(
                    Diagnostic::new("MF2E030", "unknown formatter")
                        .with_span(spec.key.clone(), var.span.line, var.span.column),
                );
            } else if !formatter_accepts_arg(formatter, &arg.arg_type) {
                diagnostics.push(
                    Diagnostic::new("MF2E021", "variable type mismatch")
                        .with_span(spec.key.clone(), var.span.line, var.span.column),
                );
            }
        }
    } else {
        diagnostics.push(
            Diagnostic::new("MF2E020", "unknown variable")
                .with_span(spec.key.clone(), var.span.line, var.span.column),
        );
    }
}

fn validate_select(select: &SelectExpr, spec: &MessageSpec, diagnostics: &mut Vec<Diagnostic>) {
    let has_other = select
        .cases
        .iter()
        .any(|case| matches!(case.key, CaseKey::Other) || case.is_default);
    if !has_other {
        diagnostics.push(
            Diagnostic::new("MF2E010", "missing required other case")
                .with_span(spec.key.clone(), select.span.line, select.span.column),
        );
    }
    if let Some(arg) = spec.args.iter().find(|arg| arg.name == select.selector) {
        let required = match select.kind {
            SelectKind::Select => ArgType::String,
            SelectKind::Plural => ArgType::Number,
        };
        if arg.arg_type != ArgType::Any && arg.arg_type != required {
            diagnostics.push(
                Diagnostic::new("MF2E021", "variable type mismatch")
                    .with_span(spec.key.clone(), select.span.line, select.span.column),
            );
        }
    } else {
        diagnostics.push(
            Diagnostic::new("MF2E020", "unknown variable")
                .with_span(spec.key.clone(), select.span.line, select.span.column),
        );
    }

    for case in &select.cases {
        validate_segments(&case.value.segments, spec, diagnostics);
    }
}

fn is_known_formatter(name: &str) -> bool {
    matches!(
        name,
        "number" | "date" | "time" | "datetime" | "unit" | "currency" | "identity"
    )
}

fn formatter_accepts_arg(formatter: &str, arg_type: &ArgType) -> bool {
    match formatter {
        "number" => matches!(arg_type, ArgType::Number | ArgType::Any),
        "date" | "time" | "datetime" => matches!(arg_type, ArgType::DateTime | ArgType::Any),
        "unit" => matches!(arg_type, ArgType::Unit | ArgType::Any),
        "currency" => matches!(arg_type, ArgType::Currency | ArgType::Any),
        "identity" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_message, ArgType, MessageSpec};
    use crate::model::ArgSpec;
    use crate::parser::parse_message;

    fn spec(args: Vec<ArgSpec>) -> MessageSpec {
        MessageSpec {
            key: "test".to_string(),
            args,
        }
    }

    #[test]
    fn reports_unknown_variable() {
        let message = parse_message("{ $name }").expect("parse");
        let diagnostics = validate_message(&message, &spec(vec![]));
        assert!(diagnostics.iter().any(|d| d.code == "MF2E020"));
    }

    #[test]
    fn reports_missing_other_case() {
        let message = parse_message("{ $count -> [one] {1} }").expect("parse");
        let diagnostics = validate_message(
            &message,
            &spec(vec![ArgSpec {
                name: "count".to_string(),
                arg_type: ArgType::Number,
                required: true,
            }]),
        );
        assert!(diagnostics.iter().any(|d| d.code == "MF2E010"));
    }

    #[test]
    fn reports_unknown_formatter() {
        let message = parse_message("{ $value :weird }").expect("parse");
        let diagnostics = validate_message(
            &message,
            &spec(vec![ArgSpec {
                name: "value".to_string(),
                arg_type: ArgType::String,
                required: true,
            }]),
        );
        assert!(diagnostics.iter().any(|d| d.code == "MF2E030"));
    }

    #[test]
    fn reports_type_mismatch() {
        let message = parse_message("{ $value :number }").expect("parse");
        let diagnostics = validate_message(
            &message,
            &spec(vec![ArgSpec {
                name: "value".to_string(),
                arg_type: ArgType::String,
                required: true,
            }]),
        );
        assert!(diagnostics.iter().any(|d| d.code == "MF2E021"));
    }
}

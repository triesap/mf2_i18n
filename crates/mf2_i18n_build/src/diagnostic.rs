use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub related_paths: Vec<String>,
    pub suggestions: Vec<String>,
}

impl Diagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            related_paths: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn with_severity(mut self, severity: DiagnosticSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_span(mut self, file: impl Into<String>, line: u32, column: u32) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self
    }

    pub fn with_related_path(mut self, path: impl Into<String>) -> Self {
        self.related_paths.push(path.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, DiagnosticReport, DiagnosticSeverity};

    #[test]
    fn serializes_structured_diagnostic_report() {
        let report = DiagnosticReport::new(vec![
            Diagnostic::new("MF2E001", "parse error")
                .with_severity(DiagnosticSeverity::Error)
                .with_span("locales/en/common.mf2", 3, 1)
                .with_related_path("locales/es/common.mf2")
                .with_suggestion("Check the MF2 message syntax."),
        ]);

        let value = serde_json::to_value(report).expect("json");
        assert_eq!(value["diagnostics"][0]["code"], "MF2E001");
        assert_eq!(value["diagnostics"][0]["severity"], "error");
        assert_eq!(value["diagnostics"][0]["file"], "locales/en/common.mf2");
        assert_eq!(
            value["diagnostics"][0]["related_paths"][0],
            "locales/es/common.mf2"
        );
        assert_eq!(
            value["diagnostics"][0]["suggestions"][0],
            "Check the MF2 message syntax."
        );
    }
}

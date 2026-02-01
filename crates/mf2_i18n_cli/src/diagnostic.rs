#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

impl Diagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            file: None,
            line: None,
            column: None,
        }
    }

    pub fn with_span(
        mut self,
        file: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

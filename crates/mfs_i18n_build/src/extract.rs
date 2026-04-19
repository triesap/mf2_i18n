use crate::lexer::Span;
use crate::model::{ArgSpec, ArgType};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ExtractedMessage {
    pub key: String,
    pub args: Vec<ArgSpec>,
}

#[derive(Debug, Clone, Error)]
#[error("{message}")]
pub struct ExtractError {
    pub message: String,
    pub span: Span,
}

pub fn extract_messages(input: &str) -> Result<Vec<ExtractedMessage>, ExtractError> {
    let mut scanner = Scanner::new(input);
    let mut messages = Vec::new();
    while let Some(byte) = scanner.peek() {
        if scanner.starts_line_comment() {
            scanner.skip_line_comment();
            continue;
        }
        if scanner.starts_block_comment() {
            scanner.skip_block_comment();
            continue;
        }
        if scanner.starts_raw_string() {
            scanner.skip_raw_string()?;
            continue;
        }
        if byte == b'"' {
            scanner.skip_string()?;
            continue;
        }
        if scanner.starts_t_macro() {
            let message = scanner.parse_t_macro()?;
            messages.push(message);
            continue;
        }
        scanner.bump();
    }
    Ok(messages)
}

struct Scanner<'a> {
    input: &'a [u8],
    index: usize,
    line: u32,
    column: u32,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.input.get(self.index + 1).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(byte)
    }

    fn span(&self, start: usize, end: usize, line: u32, column: u32) -> Span {
        Span {
            start,
            end,
            line,
            column,
        }
    }

    fn error(&self, message: &str, start: usize, line: u32, column: u32) -> ExtractError {
        ExtractError {
            message: message.to_string(),
            span: self.span(start, self.index, line, column),
        }
    }

    fn starts_line_comment(&self) -> bool {
        self.peek() == Some(b'/') && self.peek_next() == Some(b'/')
    }

    fn starts_block_comment(&self) -> bool {
        self.peek() == Some(b'/') && self.peek_next() == Some(b'*')
    }

    fn starts_raw_string(&self) -> bool {
        if self.peek() != Some(b'r') {
            return false;
        }
        let mut idx = self.index + 1;
        while let Some(b'#') = self.input.get(idx).copied() {
            idx += 1;
        }
        self.input.get(idx) == Some(&b'"')
    }

    fn starts_t_macro(&self) -> bool {
        if self.peek() != Some(b't') || self.peek_next() != Some(b'!') {
            return false;
        }
        if self.index > 0 {
            if let Some(prev) = self.input.get(self.index - 1).copied() {
                if is_ident_continue(prev) {
                    return false;
                }
            }
        }
        true
    }

    fn skip_line_comment(&mut self) {
        while let Some(byte) = self.bump() {
            if byte == b'\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        self.bump();
        self.bump();
        while let Some(byte) = self.bump() {
            if byte == b'*' && self.peek() == Some(b'/') {
                self.bump();
                break;
            }
        }
    }

    fn skip_string(&mut self) -> Result<(), ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        self.bump();
        while let Some(byte) = self.bump() {
            match byte {
                b'\\' => {
                    self.bump();
                }
                b'"' => return Ok(()),
                _ => {}
            }
        }
        Err(self.error("unterminated string literal", start, line, column))
    }

    fn skip_raw_string(&mut self) -> Result<(), ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        self.bump();
        let mut hashes = 0;
        while self.peek() == Some(b'#') {
            hashes += 1;
            self.bump();
        }
        if self.peek() != Some(b'"') {
            return Err(self.error("invalid raw string", start, line, column));
        }
        self.bump();
        loop {
            if self.peek().is_none() {
                return Err(self.error("unterminated raw string", start, line, column));
            }
            if self.peek() == Some(b'"') {
                self.bump();
                let mut matched = true;
                for _ in 0..hashes {
                    if self.peek() == Some(b'#') {
                        self.bump();
                    } else {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Ok(());
                }
            } else {
                self.bump();
            }
        }
    }

    fn parse_t_macro(&mut self) -> Result<ExtractedMessage, ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        self.bump();
        self.bump();
        self.skip_ws();
        if self.peek() != Some(b'(') {
            return Err(self.error("expected '(' after t!", start, line, column));
        }
        self.bump();
        self.skip_ws();
        if self.peek() != Some(b'"') {
            return Err(self.error("expected string literal key", start, line, column));
        }
        let key = self.parse_string_value()?;
        self.skip_ws();
        let mut args = Vec::new();
        if self.peek() == Some(b',') {
            self.bump();
            loop {
                self.skip_ws();
                if self.peek() == Some(b')') {
                    break;
                }
                let name = self.parse_ident()?;
                self.skip_ws();
                if self.peek() != Some(b':') {
                    return Err(self.error(
                        "expected ':' after argument name",
                        start,
                        line,
                        column,
                    ));
                }
                self.bump();
                self.skip_ws();
                let arg_type = self.parse_arg_type()?;
                args.push(ArgSpec {
                    name,
                    arg_type,
                    required: true,
                });
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.bump();
                    }
                    Some(b')') => break,
                    _ => {
                        return Err(self.error(
                            "expected ',' or ')' after argument",
                            start,
                            line,
                            column,
                        ));
                    }
                }
            }
        }
        self.skip_ws();
        if self.peek() != Some(b')') {
            return Err(self.error("expected ')' to close t! macro", start, line, column));
        }
        self.bump();
        Ok(ExtractedMessage { key, args })
    }

    fn parse_string_value(&mut self) -> Result<String, ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        if self.peek() != Some(b'"') {
            return Err(self.error("expected string literal", start, line, column));
        }
        self.bump();
        let mut out = String::new();
        while let Some(byte) = self.bump() {
            match byte {
                b'\\' => {
                    if let Some(next) = self.bump() {
                        out.push(next as char);
                    }
                }
                b'"' => return Ok(out),
                _ => out.push(byte as char),
            }
        }
        Err(self.error("unterminated string literal", start, line, column))
    }

    fn parse_ident(&mut self) -> Result<String, ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        let first = self
            .peek()
            .ok_or_else(|| self.error("unexpected eof", start, line, column))?;
        if !is_ident_start(first) {
            return Err(self.error("expected identifier", start, line, column));
        }
        let mut out = String::new();
        out.push(first as char);
        self.bump();
        while let Some(byte) = self.peek() {
            if !is_ident_continue(byte) {
                break;
            }
            out.push(byte as char);
            self.bump();
        }
        Ok(out)
    }

    fn parse_arg_type(&mut self) -> Result<ArgType, ExtractError> {
        let start = self.index;
        let line = self.line;
        let column = self.column;
        let ident = self.parse_ident()?;
        match ident.to_ascii_lowercase().as_str() {
            "string" | "str" => Ok(ArgType::String),
            "number" | "num" => Ok(ArgType::Number),
            "bool" | "boolean" => Ok(ArgType::Bool),
            "datetime" | "date_time" => Ok(ArgType::DateTime),
            "unit" => Ok(ArgType::Unit),
            "currency" => Ok(ArgType::Currency),
            "any" => Ok(ArgType::Any),
            _ => Err(self.error("unknown argument type", start, line, column)),
        }
    }

    fn skip_ws(&mut self) {
        while let Some(byte) = self.peek() {
            if byte.is_ascii_whitespace() {
                self.bump();
            } else {
                break;
            }
        }
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::extract_messages;

    #[test]
    fn extracts_simple_key() {
        let input = r#"
        fn demo() {
            let _ = t!("home.title");
        }
        "#;
        let messages = extract_messages(input).expect("extract");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].key, "home.title");
    }

    #[test]
    fn extracts_args() {
        let input = r#"
        fn demo() {
            let _ = t!("cart.items", count: number, name: string);
        }
        "#;
        let messages = extract_messages(input).expect("extract");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].args.len(), 2);
        assert_eq!(messages[0].args[0].name, "count");
    }

    #[test]
    fn skips_comments_and_strings() {
        let input = r#"
        // t!("ignored")
        let s = "t!(\"nope\")";
        let _ = t!("ok");
        "#;
        let messages = extract_messages(input).expect("extract");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].key, "ok");
    }
}

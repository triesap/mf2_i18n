#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Text(String),
    LBrace,
    RBrace,
    Dollar,
    Colon,
    Equals,
    Comma,
    Ident(String),
    Number(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub span: Span,
}

pub struct Lexer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    offset: usize,
    line: u32,
    column: u32,
    in_expr: bool,
    expr_depth: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            offset: 0,
            line: 1,
            column: 1,
            in_expr: false,
            expr_depth: 0,
        }
    }

    pub fn lex_all(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while self.offset < self.bytes.len() {
            if self.in_expr {
                self.lex_expr_token(&mut tokens)?;
            } else {
                self.lex_text_token(&mut tokens)?;
            }
        }
        if self.expr_depth > 0 {
            let span = Span {
                start: self.offset,
                end: self.offset,
                line: self.line,
                column: self.column,
            };
            return Err(self.error("unclosed brace", span));
        }
        Ok(tokens)
    }

    fn lex_text_token(&mut self, tokens: &mut Vec<Token>) -> Result<(), LexError> {
        let start = self.offset;
        let line = self.line;
        let column = self.column;
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if byte == b'{' {
                if self.offset > start {
                    let text = &self.input[start..self.offset];
                    tokens.push(Token {
                        kind: TokenKind::Text(text.to_string()),
                        span: Span {
                            start,
                            end: self.offset,
                            line,
                            column,
                        },
                    });
                }
                let span = self.single_span(self.offset, line, column);
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    span,
                });
                self.advance_byte();
                self.in_expr = true;
                self.expr_depth = 1;
                return Ok(());
            }
            self.advance_byte();
        }
        if self.offset > start {
            let text = &self.input[start..self.offset];
            tokens.push(Token {
                kind: TokenKind::Text(text.to_string()),
                span: Span {
                    start,
                    end: self.offset,
                    line,
                    column,
                },
            });
        }
        Ok(())
    }

    fn lex_expr_token(&mut self, tokens: &mut Vec<Token>) -> Result<(), LexError> {
        self.skip_whitespace();
        if self.offset >= self.bytes.len() {
            return Ok(());
        }
        let byte = self.bytes[self.offset];
        let line = self.line;
        let column = self.column;
        let span = self.single_span(self.offset, line, column);
        match byte {
            b'}' => {
                if self.expr_depth == 0 {
                    return Err(self.error("unbalanced brace", span));
                }
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    span,
                });
                self.advance_byte();
                self.expr_depth -= 1;
                if self.expr_depth == 0 {
                    self.in_expr = false;
                }
            }
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    span,
                });
                self.advance_byte();
                self.expr_depth += 1;
            }
            b'$' => {
                tokens.push(Token {
                    kind: TokenKind::Dollar,
                    span,
                });
                self.advance_byte();
            }
            b':' => {
                tokens.push(Token {
                    kind: TokenKind::Colon,
                    span,
                });
                self.advance_byte();
            }
            b'=' => {
                tokens.push(Token {
                    kind: TokenKind::Equals,
                    span,
                });
                self.advance_byte();
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    span,
                });
                self.advance_byte();
            }
            b'-' | b'0'..=b'9' => {
                let token = self.lex_number()?;
                tokens.push(token);
            }
            _ => {
                if is_ident_start(byte) {
                    let token = self.lex_ident()?;
                    tokens.push(token);
                } else {
                    return Err(self.error("unexpected character", span));
                }
            }
        }
        Ok(())
    }

    fn lex_ident(&mut self) -> Result<Token, LexError> {
        let start = self.offset;
        let line = self.line;
        let column = self.column;
        self.advance_byte();
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if is_ident_continue(byte) {
                self.advance_byte();
            } else {
                break;
            }
        }
        let ident = &self.input[start..self.offset];
        Ok(Token {
            kind: TokenKind::Ident(ident.to_string()),
            span: Span {
                start,
                end: self.offset,
                line,
                column,
            },
        })
    }

    fn lex_number(&mut self) -> Result<Token, LexError> {
        let start = self.offset;
        let line = self.line;
        let column = self.column;
        if self.bytes[self.offset] == b'-' {
            self.advance_byte();
        }
        let mut saw_digit = false;
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if byte.is_ascii_digit() {
                saw_digit = true;
                self.advance_byte();
            } else {
                break;
            }
        }
        if self.offset < self.bytes.len() && self.bytes[self.offset] == b'.' {
            self.advance_byte();
            while self.offset < self.bytes.len() && self.bytes[self.offset].is_ascii_digit() {
                saw_digit = true;
                self.advance_byte();
            }
        }
        if !saw_digit {
            let span = Span {
                start,
                end: self.offset,
                line,
                column,
            };
            return Err(self.error("invalid number", span));
        }
        let value = &self.input[start..self.offset];
        Ok(Token {
            kind: TokenKind::Number(value.to_string()),
            span: Span {
                start,
                end: self.offset,
                line,
                column,
            },
        })
    }

    fn skip_whitespace(&mut self) {
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            if byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n' {
                self.advance_byte();
            } else {
                break;
            }
        }
    }

    fn advance_byte(&mut self) {
        let byte = self.bytes[self.offset];
        self.offset += 1;
        if byte == b'\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
    }

    fn single_span(&self, start: usize, line: u32, column: u32) -> Span {
        Span {
            start,
            end: start + 1,
            line,
            column,
        }
    }

    fn error(&self, message: &str, span: Span) -> LexError {
        LexError {
            message: message.to_string(),
            span,
        }
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit() || byte == b'-'
}

#[cfg(test)]
mod tests {
    use super::{Lexer, TokenKind};

    #[test]
    fn lexes_text_and_expr_tokens() {
        let input = "Hello { $name }";
        let tokens = Lexer::new(input).lex_all().expect("lex");
        assert_eq!(tokens.len(), 5);
        assert!(matches!(tokens[0].kind, TokenKind::Text(_)));
        assert_eq!(tokens[1].kind, TokenKind::LBrace);
        assert_eq!(tokens[2].kind, TokenKind::Dollar);
        match &tokens[3].kind {
            TokenKind::Ident(value) => assert_eq!(value, "name"),
            _ => panic!("expected ident"),
        }
        assert_eq!(tokens[4].kind, TokenKind::RBrace);
    }

    #[test]
    fn lexes_numbers_and_equals() {
        let input = "{ =0 {zero} }";
        let tokens = Lexer::new(input).lex_all().expect("lex");
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Equals));
        assert!(tokens.iter().any(|token| matches!(token.kind, TokenKind::Number(_))));
    }

    #[test]
    fn lexes_colon_and_ident() {
        let input = "{ $value :number }";
        let tokens = Lexer::new(input).lex_all().expect("lex");
        assert!(tokens.iter().any(|token| token.kind == TokenKind::Colon));
        assert!(tokens.iter().any(|token| matches!(token.kind, TokenKind::Ident(_))));
    }
}

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
    Arrow,
    Dollar,
    Colon,
    Equals,
    Comma,
    LBracket,
    RBracket,
    Star,
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
    mode_stack: Vec<Mode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Text,
    Expr,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            offset: 0,
            line: 1,
            column: 1,
            mode_stack: vec![Mode::Text],
        }
    }

    pub fn lex_all(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();
        while self.offset < self.bytes.len() {
            if self.is_expr_mode() {
                self.lex_expr_token(&mut tokens)?;
            } else {
                self.lex_text_token(&mut tokens)?;
            }
        }
        if self.mode_stack.len() > 1 {
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
            if byte == b'{' || byte == b'}' {
                break;
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
        if self.offset >= self.bytes.len() {
            return Ok(());
        }
        let byte = self.bytes[self.offset];
        let line = self.line;
        let column = self.column;
        let span = self.single_span(self.offset, line, column);
        match byte {
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    span,
                });
                self.advance_byte();
                self.mode_stack.push(Mode::Expr);
            }
            b'}' => {
                if self.mode_stack.len() <= 1 {
                    return Err(self.error("unbalanced brace", span));
                }
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    span,
                });
                self.advance_byte();
                self.mode_stack.pop();
            }
            _ => {}
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
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    span,
                });
                self.advance_byte();
                self.mode_stack.pop();
            }
            b'{' => {
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    span,
                });
                self.advance_byte();
                self.mode_stack.push(Mode::Text);
            }
            b'[' => {
                tokens.push(Token {
                    kind: TokenKind::LBracket,
                    span,
                });
                self.advance_byte();
            }
            b']' => {
                tokens.push(Token {
                    kind: TokenKind::RBracket,
                    span,
                });
                self.advance_byte();
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
            b'*' => {
                tokens.push(Token {
                    kind: TokenKind::Star,
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
            b'-' => {
                if self.peek_byte() == Some(b'>') {
                    let arrow_span = Span {
                        start: self.offset,
                        end: self.offset + 2,
                        line,
                        column,
                    };
                    tokens.push(Token {
                        kind: TokenKind::Arrow,
                        span: arrow_span,
                    });
                    self.advance_byte();
                    self.advance_byte();
                } else {
                    let token = self.lex_number()?;
                    tokens.push(token);
                }
            }
            b'0'..=b'9' => {
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

    fn peek_byte(&self) -> Option<u8> {
        self.bytes.get(self.offset + 1).copied()
    }

    fn is_expr_mode(&self) -> bool {
        matches!(self.mode_stack.last(), Some(Mode::Expr))
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

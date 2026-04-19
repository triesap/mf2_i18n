use crate::lexer::{LexError, Lexer, Span, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Text { value: String, span: Span },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Variable(VarExpr),
    Select(SelectExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VarExpr {
    pub name: String,
    pub formatter: Option<FormatterExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatterExpr {
    pub name: String,
    pub options: Vec<FormatterOptionExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatterOptionExpr {
    pub key: String,
    pub value: FormatterOptionExprValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FormatterOptionExprValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectKind {
    Select,
    Plural,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectExpr {
    pub selector: String,
    pub cases: Vec<SelectCase>,
    pub kind: SelectKind,
    pub formatter: Option<FormatterExpr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectCase {
    pub key: CaseKey,
    pub value: Message,
    pub is_default: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseKey {
    Ident(String),
    Exact(u32),
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        Self {
            message: error.message,
            span: error.span,
        }
    }
}

pub fn parse_message(input: &str) -> Result<Message, ParseError> {
    let tokens = Lexer::new(input).lex_all()?;
    let mut parser = Parser::new(tokens);
    parser.parse_message(false)
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse_message(&mut self, stop_on_rbrace: bool) -> Result<Message, ParseError> {
        let mut segments = Vec::new();
        while let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Text(value) => {
                    let token = self.next().expect("token");
                    segments.push(Segment::Text {
                        value,
                        span: token.span,
                    });
                }
                TokenKind::LBrace => {
                    self.next();
                    let expr = self.parse_expr()?;
                    segments.push(Segment::Expr(expr));
                }
                TokenKind::RBrace if stop_on_rbrace => break,
                TokenKind::RBrace => {
                    return Err(self.error("unexpected closing brace", token.span));
                }
                _ => {
                    return Err(self.error("unexpected token in message", token.span));
                }
            }
        }
        Ok(Message { segments })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let start = self.peek_span().unwrap_or_else(|| Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        });
        self.expect(TokenKind::Dollar)?;
        let name = self.expect_ident()?;
        let formatter = if self.peek_is(&TokenKind::Colon) {
            self.next();
            Some(self.parse_formatter_expr()?)
        } else {
            None
        };
        if self.peek_is(&TokenKind::Arrow) {
            self.next();
            let cases = self.parse_cases()?;
            let end = self.expect(TokenKind::RBrace)?;
            let mut kind = SelectKind::Select;
            if formatter.as_ref().map(|formatter| formatter.name.as_str()) == Some("plural") {
                kind = SelectKind::Plural;
            }
            if cases
                .iter()
                .any(|case| matches!(case.key, CaseKey::Exact(_)))
            {
                kind = SelectKind::Plural;
            }
            Ok(Expr::Select(SelectExpr {
                selector: name,
                cases,
                kind,
                formatter,
                span: span_merge(start, end.span),
            }))
        } else {
            let end = self.expect(TokenKind::RBrace)?;
            Ok(Expr::Variable(VarExpr {
                name,
                formatter,
                span: span_merge(start, end.span),
            }))
        }
    }

    fn parse_formatter_expr(&mut self) -> Result<FormatterExpr, ParseError> {
        let name = self.expect_ident()?;
        let mut options = Vec::new();
        loop {
            if self.peek_is(&TokenKind::Comma) {
                self.next();
                continue;
            }
            if self.peek_is(&TokenKind::Arrow) || self.peek_is(&TokenKind::RBrace) {
                break;
            }
            let Some(token) = self.peek().cloned() else {
                break;
            };
            let TokenKind::Ident(key) = token.kind else {
                return Err(self.error("expected formatter option", token.span));
            };
            self.next();
            self.expect(TokenKind::Equals)?;
            let value = self.parse_formatter_option_value()?;
            options.push(FormatterOptionExpr { key, value });
        }
        Ok(FormatterExpr { name, options })
    }

    fn parse_formatter_option_value(&mut self) -> Result<FormatterOptionExprValue, ParseError> {
        let token = self.next().ok_or_else(|| {
            self.error(
                "unexpected eof",
                Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            )
        })?;
        match token.kind {
            TokenKind::Ident(value) => match value.as_str() {
                "true" => Ok(FormatterOptionExprValue::Bool(true)),
                "false" => Ok(FormatterOptionExprValue::Bool(false)),
                _ => Ok(FormatterOptionExprValue::Str(value)),
            },
            TokenKind::Number(value) => {
                let value = value
                    .parse::<f64>()
                    .map_err(|_| self.error("invalid formatter option number", token.span))?;
                Ok(FormatterOptionExprValue::Num(value))
            }
            _ => Err(self.error("expected formatter option value", token.span)),
        }
    }

    fn parse_cases(&mut self) -> Result<Vec<SelectCase>, ParseError> {
        let mut cases = Vec::new();
        while let Some(token) = self.peek() {
            if matches!(token.kind, TokenKind::RBrace) {
                break;
            }
            let is_default = if self.peek_is(&TokenKind::Star) {
                self.next();
                true
            } else {
                false
            };
            self.expect(TokenKind::LBracket)?;
            let key = self.parse_case_key()?;
            let key_span = self.expect(TokenKind::RBracket)?.span;
            self.expect(TokenKind::LBrace)?;
            let value = self.parse_message(true)?;
            let end_span = self.expect(TokenKind::RBrace)?.span;
            cases.push(SelectCase {
                key,
                value,
                is_default,
                span: span_merge(key_span, end_span),
            });
        }
        Ok(cases)
    }

    fn parse_case_key(&mut self) -> Result<CaseKey, ParseError> {
        if self.peek_is(&TokenKind::Equals) {
            self.next();
            let number = self.expect_number()?;
            let value = number
                .parse::<u32>()
                .map_err(|_| self.error("invalid exact number", self.peek_span().unwrap()))?;
            return Ok(CaseKey::Exact(value));
        }
        if let Some(token) = self.peek().cloned() {
            match token.kind {
                TokenKind::Ident(value) => {
                    self.next();
                    if value == "other" {
                        return Ok(CaseKey::Other);
                    }
                    return Ok(CaseKey::Ident(value));
                }
                TokenKind::Number(value) => {
                    self.next();
                    return Ok(CaseKey::Ident(value));
                }
                _ => {}
            }
        }
        Err(self.error("expected case key", self.peek_span().unwrap()))
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let token = self.next().ok_or_else(|| {
            self.error(
                "unexpected eof",
                Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            )
        })?;
        if token.kind == kind {
            Ok(token)
        } else {
            Err(self.error("unexpected token", token.span))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        let token = self.next().ok_or_else(|| {
            self.error(
                "unexpected eof",
                Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            )
        })?;
        match token.kind {
            TokenKind::Ident(value) => Ok(value),
            _ => Err(self.error("expected identifier", token.span)),
        }
    }

    fn expect_number(&mut self) -> Result<String, ParseError> {
        let token = self.next().ok_or_else(|| {
            self.error(
                "unexpected eof",
                Span {
                    start: 0,
                    end: 0,
                    line: 1,
                    column: 1,
                },
            )
        })?;
        match token.kind {
            TokenKind::Number(value) => Ok(value),
            _ => Err(self.error("expected number", token.span)),
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek_is(&self, kind: &TokenKind) -> bool {
        self.peek()
            .map(|token| &token.kind == kind)
            .unwrap_or(false)
    }

    fn peek_span(&self) -> Option<Span> {
        self.peek().map(|token| token.span.clone())
    }

    fn error(&self, message: &str, span: Span) -> ParseError {
        ParseError {
            message: message.to_string(),
            span,
        }
    }
}

fn span_merge(start: Span, end: Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
        line: start.line,
        column: start.column,
    }
}

#[cfg(test)]
mod tests {
    use super::{CaseKey, Expr, FormatterOptionExprValue, Segment, SelectKind, parse_message};

    #[test]
    fn parses_variable_expression() {
        let message = parse_message("Hello { $name }").expect("parse");
        assert_eq!(message.segments.len(), 2);
        match &message.segments[1] {
            Segment::Expr(Expr::Variable(expr)) => {
                assert_eq!(expr.name, "name");
                assert_eq!(expr.formatter, None);
            }
            _ => panic!("expected variable expr"),
        }
    }

    #[test]
    fn parses_formatter_call() {
        let message = parse_message("{ $value :number }").expect("parse");
        match &message.segments[0] {
            Segment::Expr(Expr::Variable(expr)) => {
                assert_eq!(
                    expr.formatter
                        .as_ref()
                        .map(|formatter| formatter.name.as_str()),
                    Some("number")
                );
            }
            _ => panic!("expected variable expr"),
        }
    }

    #[test]
    fn parses_formatter_options() {
        let message = parse_message(
            "{ $value :number style=percent minimum-fraction-digits=2 use-grouping=true }",
        )
        .expect("parse");
        match &message.segments[0] {
            Segment::Expr(Expr::Variable(expr)) => {
                let formatter = expr.formatter.as_ref().expect("formatter");
                assert_eq!(formatter.name, "number");
                assert_eq!(formatter.options.len(), 3);
                assert_eq!(formatter.options[0].key, "style");
                assert_eq!(
                    formatter.options[0].value,
                    FormatterOptionExprValue::Str("percent".to_string())
                );
                assert_eq!(formatter.options[1].key, "minimum-fraction-digits");
                assert_eq!(
                    formatter.options[1].value,
                    FormatterOptionExprValue::Num(2.0)
                );
                assert_eq!(formatter.options[2].key, "use-grouping");
                assert_eq!(
                    formatter.options[2].value,
                    FormatterOptionExprValue::Bool(true)
                );
            }
            _ => panic!("expected variable expr"),
        }
    }

    #[test]
    fn parses_select_cases() {
        let message = parse_message("{ $count -> [one] {1} *[other] {n} }").expect("parse");
        match &message.segments[0] {
            Segment::Expr(Expr::Select(expr)) => {
                assert_eq!(expr.kind, SelectKind::Select);
                assert_eq!(expr.cases.len(), 2);
                assert!(matches!(expr.cases[0].key, CaseKey::Ident(_)));
                assert!(expr.cases[1].is_default);
            }
            _ => panic!("expected select expr"),
        }
    }
}

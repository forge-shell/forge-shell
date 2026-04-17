#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

/// A position in the source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub col: usize,
}

/// The 'kind' of a token: what it represents in the grammar.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // --- Literals ---
    Integer(i64),
    Float(f64),
    StringLit(String),
    Bool(bool),

    // --- Identifiers and Keywords ---
    Ident(String),
    Fn,
    Let,
    If,
    Else,
    Return,
    Import,
    Export,

    // --- Arithmetic operators ---
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // --- Comparison operators ---
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // --- Logical operators ---
    And,
    Or,
    Not,

    // --- Assignment ---
    Assign,

    // --- Shell operators ---
    Pipe,
    Amp,
    RedirectOut,
    RedirectAppend,
    RedirectIn,

    // --- Delimiters ---
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    DotDot,
    Arrow,
    FatArrow,

    // --- Special ---
    Dollar,
    At,
    Newline,
    Eof,
}

/// A token with its location in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub kind: TokenKind,
    pub span: Span,
}

impl SpannedToken {
    #[must_use]
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_span() -> Span {
        Span {
            start: 0,
            end: 10,
            line: 1,
            col: 1,
        }
    }

    #[test]
    fn test_integer_token_roundtrip() {
        let token = SpannedToken::new(TokenKind::Integer(42), make_span());
        assert_eq!(token.kind, TokenKind::Integer(42));
    }

    #[test]
    fn test_float_token_roundtrip() {
        let token = SpannedToken::new(TokenKind::Float(3.14), make_span());
        assert_eq!(token.kind, TokenKind::Float(3.14));
    }

    #[test]
    fn test_string_token_roundtrip() {
        let token = SpannedToken::new(TokenKind::StringLit("hello".to_string()), make_span());
        assert_eq!(token.kind, TokenKind::StringLit("hello".to_string()));
    }

    #[test]
    fn test_bool_token_roundtrip() {
        let t = SpannedToken::new(TokenKind::Bool(true), make_span());
        let f = SpannedToken::new(TokenKind::Bool(false), make_span());
        assert_eq!(t.kind, TokenKind::Bool(true));
        assert_eq!(f.kind, TokenKind::Bool(false));
    }

    #[test]
    fn test_keyword_variants() {
        let keywords = vec![
            TokenKind::Fn,
            TokenKind::Let,
            TokenKind::If,
            TokenKind::Else,
            TokenKind::Return,
            TokenKind::Import,
            TokenKind::Export,
        ];
        for kw in keywords {
            let token = SpannedToken::new(kw.clone(), make_span());
            assert_eq!(token.kind, kw);
        }
    }

    #[test]
    fn test_operator_variants() {
        let ops = vec![
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::EqEq,
            TokenKind::Ne,
            TokenKind::And,
            TokenKind::Or,
            TokenKind::Pipe,
        ];
        for op in ops {
            let token = SpannedToken::new(op.clone(), make_span());
            assert_eq!(token.kind, op);
        }
    }

    #[test]
    fn test_span_fields() {
        let span = Span {
            start: 5,
            end: 10,
            line: 3,
            col: 7,
        };
        assert_eq!(span.start, 5);
        assert_eq!(span.end, 10);
        assert_eq!(span.line, 3);
        assert_eq!(span.col, 7);
    }

    #[test]
    fn test_eof_token() {
        let token = SpannedToken::new(TokenKind::Eof, make_span());
        assert_eq!(token.kind, TokenKind::Eof);
    }
}

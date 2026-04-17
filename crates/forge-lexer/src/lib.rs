#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod error;
mod lexer;

pub use error::LexError;
pub use lexer::Lexer;

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

    // --- Shebang directive ---
    ShebangDirective(String),
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
        let token = SpannedToken::new(TokenKind::Float(std::f64::consts::PI), make_span());
        assert_eq!(token.kind, TokenKind::Float(std::f64::consts::PI));
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

    fn tokenise(src: &str) -> Vec<TokenKind> {
        Lexer::new(src)
            .tokenise()
            .expect("tokenise failed")
            .into_iter()
            .map(|t| t.kind)
            .filter(|k| k != &TokenKind::Eof)
            .collect()
    }

    #[test]
    fn test_empty_source() {
        let tokens = Lexer::new("").tokenise().unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn test_integer() {
        assert_eq!(tokenise("42"), vec![TokenKind::Integer(42)]);
    }

    #[test]
    fn test_float() {
        assert_eq!(tokenise("3.14"), vec![TokenKind::Float(3.14)]);
    }

    #[test]
    fn test_string() {
        assert_eq!(
            tokenise(r#""hello""#),
            vec![TokenKind::StringLit("hello".to_string())]
        );
    }

    #[test]
    fn test_string_escape_sequences() {
        assert_eq!(
            tokenise(r#""\n\t\"\\""#),
            vec![TokenKind::StringLit("\n\t\"\\".to_string())]
        );
    }

    #[test]
    fn test_bool() {
        assert_eq!(tokenise("true"), vec![TokenKind::Bool(true)]);
        assert_eq!(tokenise("false"), vec![TokenKind::Bool(false)]);
    }

    #[test]
    fn test_keywords() {
        assert_eq!(tokenise("fn"), vec![TokenKind::Fn]);
        assert_eq!(tokenise("let"), vec![TokenKind::Let]);
        assert_eq!(tokenise("if"), vec![TokenKind::If]);
        assert_eq!(tokenise("else"), vec![TokenKind::Else]);
        assert_eq!(tokenise("return"), vec![TokenKind::Return]);
        assert_eq!(tokenise("import"), vec![TokenKind::Import]);
        assert_eq!(tokenise("export"), vec![TokenKind::Export]);
    }

    #[test]
    fn test_identifier() {
        assert_eq!(
            tokenise("my_var"),
            vec![TokenKind::Ident("my_var".to_string())]
        );
    }

    #[test]
    fn test_operators() {
        assert_eq!(tokenise("+"), vec![TokenKind::Plus]);
        assert_eq!(tokenise("=="), vec![TokenKind::EqEq]);
        assert_eq!(tokenise("!="), vec![TokenKind::Ne]);
        assert_eq!(tokenise("<="), vec![TokenKind::Le]);
        assert_eq!(tokenise(">="), vec![TokenKind::Ge]);
        assert_eq!(tokenise("->"), vec![TokenKind::Arrow]);
        assert_eq!(tokenise("=>"), vec![TokenKind::FatArrow]);
        assert_eq!(tokenise(".."), vec![TokenKind::DotDot]);
        assert_eq!(tokenise("||"), vec![TokenKind::Or]);
        assert_eq!(tokenise("&&"), vec![TokenKind::And]);
        assert_eq!(tokenise(">>"), vec![TokenKind::RedirectAppend]);
    }

    #[test]
    fn test_pipe_vs_or() {
        assert_eq!(tokenise("|"), vec![TokenKind::Pipe]);
        assert_eq!(tokenise("||"), vec![TokenKind::Or]);
    }

    #[test]
    fn test_comment_skipped() {
        assert_eq!(
            tokenise("42 # this is a comment\n99"),
            vec![
                TokenKind::Integer(42),
                TokenKind::Newline,
                TokenKind::Integer(99)
            ]
        );
    }

    #[test]
    fn test_crlf_normalised() {
        assert_eq!(
            tokenise("a\r\nb"),
            vec![
                TokenKind::Ident("a".to_string()),
                TokenKind::Newline,
                TokenKind::Ident("b".to_string()),
            ]
        );
    }

    #[test]
    fn test_multiline_span() {
        let tokens = Lexer::new("a\nb").tokenise().unwrap();
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[2].span.line, 2);
    }

    #[test]
    fn test_unterminated_string_error() {
        let result = Lexer::new(r#""unclosed"#).tokenise();
        assert!(matches!(result, Err(LexError::UnterminatedString { .. })));
    }

    #[test]
    fn test_unexpected_char_error() {
        let result = Lexer::new("§").tokenise();
        assert!(matches!(
            result,
            Err(LexError::UnexpectedChar { ch: '§', .. })
        ));
    }

    #[test]
    fn test_integer_overflow_error() {
        let result = Lexer::new("99999999999999999999999999").tokenise();
        assert!(matches!(result, Err(LexError::InvalidNumber { .. })));
    }
}

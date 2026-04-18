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

/// A segment of an interpolated string — either a literal text chunk or an embedded expression.
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    Literal(String),
    Interpolation(Vec<SpannedToken>),
}

/// The 'kind' of a token: what it represents in the grammar.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // --- Literals ---
    Integer(i64),
    Float(f64),
    /// A plain string with no interpolation: `"hello"`.
    StringLit(String),
    /// A string containing `{expr}` interpolations: `"Hello, {name}!"`.
    InterpolatedStr(Vec<StringPart>),
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
        assert_eq!(tokenise("2.56"), vec![TokenKind::Float(2.56)]);
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

    // --- String interpolation ---

    fn interp_parts(src: &str) -> Vec<StringPart> {
        match tokenise(src).into_iter().next().unwrap() {
            TokenKind::InterpolatedStr(parts) => parts,
            other => panic!("expected InterpolatedStr, got {other:?}"),
        }
    }

    fn part_literal(p: &StringPart) -> &str {
        match p {
            StringPart::Literal(s) => s,
            _ => panic!("expected Literal"),
        }
    }

    fn part_interp_kinds(p: &StringPart) -> Vec<TokenKind> {
        match p {
            StringPart::Interpolation(tokens) => tokens.iter().map(|t| t.kind.clone()).collect(),
            _ => panic!("expected Interpolation"),
        }
    }

    #[test]
    fn test_plain_string_unchanged() {
        assert_eq!(
            tokenise(r#""hello""#),
            vec![TokenKind::StringLit("hello".to_string())]
        );
    }

    #[test]
    fn test_simple_variable_interpolation() {
        let parts = interp_parts(r#""Hello, {name}!""#);
        assert_eq!(parts.len(), 3);
        assert_eq!(part_literal(&parts[0]), "Hello, ");
        assert_eq!(
            part_interp_kinds(&parts[1]),
            vec![TokenKind::Ident("name".to_string())]
        );
        assert_eq!(part_literal(&parts[2]), "!");
    }

    #[test]
    fn test_expression_interpolation() {
        let parts = interp_parts(r#""Result is {a + b}""#);
        assert_eq!(parts.len(), 3);
        assert_eq!(part_literal(&parts[0]), "Result is ");
        assert_eq!(
            part_interp_kinds(&parts[1]),
            vec![
                TokenKind::Ident("a".to_string()),
                TokenKind::Plus,
                TokenKind::Ident("b".to_string()),
            ]
        );
        assert_eq!(part_literal(&parts[2]), "");
    }

    #[test]
    fn test_method_call_interpolation() {
        let parts = interp_parts(r#""Items: {list.len()}""#);
        assert_eq!(parts.len(), 3);
        assert_eq!(part_literal(&parts[0]), "Items: ");
        assert_eq!(
            part_interp_kinds(&parts[1]),
            vec![
                TokenKind::Ident("list".to_string()),
                TokenKind::Dot,
                TokenKind::Ident("len".to_string()),
                TokenKind::LParen,
                TokenKind::RParen,
            ]
        );
        assert_eq!(part_literal(&parts[2]), "");
    }

    #[test]
    fn test_nested_braces_in_interpolation() {
        // `{if true {1} else {2}}` — braces inside expression are depth-tracked
        let parts = interp_parts(r#""{if true {1} else {2}}""#);
        assert_eq!(parts.len(), 3);
        assert_eq!(part_literal(&parts[0]), "");
        let kinds = part_interp_kinds(&parts[1]);
        assert_eq!(
            kinds,
            vec![
                TokenKind::If,
                TokenKind::Bool(true),
                TokenKind::LBrace,
                TokenKind::Integer(1),
                TokenKind::RBrace,
                TokenKind::Else,
                TokenKind::LBrace,
                TokenKind::Integer(2),
                TokenKind::RBrace,
            ]
        );
        assert_eq!(part_literal(&parts[2]), "");
    }

    #[test]
    fn test_multiple_interpolations() {
        let parts = interp_parts(r#""{a} and {b}""#);
        assert_eq!(parts.len(), 5);
        assert_eq!(part_literal(&parts[0]), "");
        assert_eq!(
            part_interp_kinds(&parts[1]),
            vec![TokenKind::Ident("a".to_string())]
        );
        assert_eq!(part_literal(&parts[2]), " and ");
        assert_eq!(
            part_interp_kinds(&parts[3]),
            vec![TokenKind::Ident("b".to_string())]
        );
        assert_eq!(part_literal(&parts[4]), "");
    }

    #[test]
    fn test_escaped_braces_not_interpolated() {
        assert_eq!(
            tokenise(r#""Set notation: {{1, 2, 3}}""#),
            vec![TokenKind::StringLit("Set notation: {1, 2, 3}".to_string())]
        );
    }

    #[test]
    fn test_unterminated_interpolation_error() {
        // No closing `}` or `"` — EOF inside the interpolation.
        let result = Lexer::new(r#""Hello, {name"#).tokenise();
        assert!(matches!(
            result,
            Err(LexError::UnterminatedInterpolation { .. })
        ));
    }
}

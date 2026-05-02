use forge_lexer::TokenKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token {got} at line {line}, expected {expected}")]
    UnexpectedToken {
        got: TokenKind,
        expected: String,
        line: usize,
    },

    #[error("unexpected end of file, expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("invalid expression starting with {got} at line {line}")]
    InvalidExpression { got: TokenKind, line: usize },

    #[error("directive '{key}' at line {line} must appear before any statements")]
    DirectiveAfterStatement { key: String, line: usize },

    #[error("invalid value '{value}' for key '{key}' at line {line}: {reason}")]
    InvalidDirectiveValue {
        key: String,
        value: String,
        reason: String,
        line: usize,
    },
}

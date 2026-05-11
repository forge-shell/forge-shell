use forge_lexer::TokenKind;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token {got} at line {line}:{col}, expected {expected}")]
    UnexpectedToken {
        got: TokenKind,
        expected: String,
        line: usize,
        col: usize,
    },

    #[error("unexpected end of file, expected {expected}")]
    UnexpectedEof { expected: String },

    #[error("invalid expression starting with {got} at line {line}:{col}")]
    InvalidExpression {
        got: TokenKind,
        line: usize,
        col: usize,
    },

    #[error("directive '{key}' at line {line}:{col} must appear before any statements")]
    DirectiveAfterStatement {
        key: String,
        line: usize,
        col: usize,
    },

    #[error("invalid value '{value}' for key '{key}' at line {line}: {reason}")]
    InvalidDirectiveValue {
        key: String,
        value: String,
        reason: String,
        line: usize,
    },
}

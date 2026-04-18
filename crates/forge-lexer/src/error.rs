use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
/// Errors that can occur during lexing.
pub enum LexError {
    /// A character was encountered that is not part of the `ForgeScript` grammar.
    #[error("unexpected character '{ch}' at line {line}, column {col}")]
    UnexpectedChar { ch: char, line: usize, col: usize },

    /// A string literal was opened but never closed before end of input.
    #[error("unterminated string starting at line {line}, column {col}")]
    UnterminatedString { line: usize, col: usize },

    /// A sequence of digits could not be parsed into a valid integer or float.
    #[error("invalid number literal '{literal}', at line {line}, column {col}")]
    InvalidNumber {
        literal: String,
        line: usize,
        col: usize,
    },

    /// A `{` interpolation inside a string was never closed with `}`.
    #[error("unterminated string interpolation in string starting at line {line}, column {col}")]
    UnterminatedInterpolation { line: usize, col: usize },
}

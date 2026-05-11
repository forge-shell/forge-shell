use std::fmt;

/// Machine-readable error code emitted by every pipeline pass.
///
/// Codes are grouped by pass in blocks of ten:
///   E001–E009  Lexer
///   E010–E019  Parser
///   E020–E029  HIR lowering
///   E030–E039  Platform backend
///   E040–E049  Runtime / executor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // ── Lexer ────────────────────────────────────────────────────────────────
    UnexpectedChar,
    UnterminatedString,
    InvalidNumber,
    UnterminatedInterpolation,

    // ── Parser ───────────────────────────────────────────────────────────────
    UnexpectedToken,
    UnexpectedEof,
    InvalidExpression,
    DirectiveAfterStatement,
    InvalidDirectiveValue,

    // ── HIR lowering ─────────────────────────────────────────────────────────
    UndefinedVariable,
    CircularImport,
    DuplicateFunctionDef,
    UnsupportedConstruct,

    // ── Platform backend ─────────────────────────────────────────────────────
    CommandNotFound,
    UnsupportedHirConstruct,
    IoError,

    // ── Runtime / executor ───────────────────────────────────────────────────
    MinVersionNotMet,
    PlatformNotSupported,
    RequiredEnvMissing,
    EnvFileNotFound,
    Timeout,
    UndefinedVariableRuntime,
    DivisionByZero,
    IntegerOverflow,
    TypeError,
}

impl ErrorCode {
    /// Numeric value of the code (e.g. `E001` → `1`).
    #[must_use]
    pub fn number(self) -> u32 {
        match self {
            Self::UnexpectedChar => 1,
            Self::UnterminatedString => 2,
            Self::InvalidNumber => 3,
            Self::UnterminatedInterpolation => 4,

            Self::UnexpectedToken => 10,
            Self::UnexpectedEof => 11,
            Self::InvalidExpression => 12,
            Self::DirectiveAfterStatement => 13,
            Self::InvalidDirectiveValue => 14,

            Self::UndefinedVariable => 20,
            Self::CircularImport => 21,
            Self::DuplicateFunctionDef => 22,
            Self::UnsupportedConstruct => 23,

            Self::CommandNotFound => 30,
            Self::UnsupportedHirConstruct => 31,
            Self::IoError => 32,

            Self::MinVersionNotMet => 40,
            Self::PlatformNotSupported => 41,
            Self::RequiredEnvMissing => 42,
            Self::EnvFileNotFound => 43,
            Self::Timeout => 44,
            Self::UndefinedVariableRuntime => 45,
            Self::DivisionByZero => 46,
            Self::IntegerOverflow => 47,
            Self::TypeError => 48,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{:03}", self.number())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_code_is_e001() {
        assert_eq!(ErrorCode::UnexpectedChar.to_string(), "E001");
    }

    #[test]
    fn last_code_is_e048() {
        assert_eq!(ErrorCode::TypeError.to_string(), "E048");
    }

    #[test]
    fn parser_codes_start_at_ten() {
        assert_eq!(ErrorCode::UnexpectedToken.number(), 10);
    }

    #[test]
    fn hir_codes_start_at_twenty() {
        assert_eq!(ErrorCode::UndefinedVariable.number(), 20);
    }

    #[test]
    fn backend_codes_start_at_thirty() {
        assert_eq!(ErrorCode::CommandNotFound.number(), 30);
    }

    #[test]
    fn runtime_codes_start_at_forty() {
        assert_eq!(ErrorCode::MinVersionNotMet.number(), 40);
    }

    #[test]
    fn display_is_zero_padded() {
        assert_eq!(ErrorCode::UnexpectedChar.to_string(), "E001");
        assert_eq!(ErrorCode::UnexpectedToken.to_string(), "E010");
        assert_eq!(ErrorCode::TypeError.to_string(), "E048");
    }
}

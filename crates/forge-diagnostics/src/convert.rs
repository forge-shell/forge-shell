use forge_backend::error::BackendError;
use forge_exec::ExecError;
use forge_hir::LowerError;
use forge_lexer::LexError;
use forge_parser::ParseError;
use forge_types::Span;

use crate::code::ErrorCode;
use crate::diagnostic::Diagnostic;

impl From<LexError> for Diagnostic {
    fn from(e: LexError) -> Self {
        match e {
            LexError::UnexpectedChar { ch, line, col } => Diagnostic::error(
                ErrorCode::UnexpectedChar,
                format!("unexpected character '{ch}'"),
                Span::point(line, col),
            ),
            LexError::UnterminatedString { line, col } => Diagnostic::error(
                ErrorCode::UnterminatedString,
                "unterminated string literal",
                Span::point(line, col),
            ),
            LexError::InvalidNumber { literal, line, col } => Diagnostic::error(
                ErrorCode::InvalidNumber,
                format!("invalid number literal '{literal}'"),
                Span::point(line, col),
            ),
            LexError::UnterminatedInterpolation { line, col } => Diagnostic::error(
                ErrorCode::UnterminatedInterpolation,
                "unterminated string interpolation — missing closing '}'",
                Span::point(line, col),
            ),
        }
    }
}

impl From<ParseError> for Diagnostic {
    fn from(e: ParseError) -> Self {
        match e {
            ParseError::UnexpectedToken {
                got,
                expected,
                line,
                col,
            } => Diagnostic::error(
                ErrorCode::UnexpectedToken,
                format!("unexpected token '{got}', expected {expected}"),
                Span::point(line, col),
            ),
            ParseError::UnexpectedEof { expected } => Diagnostic::error(
                ErrorCode::UnexpectedEof,
                format!("unexpected end of file, expected {expected}"),
                Span::default(),
            ),
            ParseError::InvalidExpression { got, line, col } => Diagnostic::error(
                ErrorCode::InvalidExpression,
                format!("invalid expression starting with '{got}'"),
                Span::point(line, col),
            ),
            ParseError::DirectiveAfterStatement { key, line, col } => Diagnostic::error(
                ErrorCode::DirectiveAfterStatement,
                format!("directive '{key}' must appear before any statements"),
                Span::point(line, col),
            )
            .with_help("move all #!forge: directives to the top of the file"),
            ParseError::InvalidDirectiveValue {
                key,
                value,
                reason,
                line,
            } => Diagnostic::error(
                ErrorCode::InvalidDirectiveValue,
                format!("invalid value '{value}' for directive '{key}': {reason}"),
                Span::point(line, 0),
            ),
        }
    }
}

impl From<LowerError> for Diagnostic {
    fn from(e: LowerError) -> Self {
        match e {
            LowerError::UndefinedVariable { name, line } => Diagnostic::error(
                ErrorCode::UndefinedVariable,
                format!("undefined variable '{name}'"),
                Span::point(line, 0),
            )
            .with_help(format!("declare it with 'let {name} = ...'")),
            LowerError::CircularImport { path } => Diagnostic::error(
                ErrorCode::CircularImport,
                format!("circular import detected: '{path}' is already being imported"),
                Span::default(),
            )
            .with_help("extract shared logic into a separate module"),
            LowerError::DuplicateFunctionDef { name } => Diagnostic::error(
                ErrorCode::DuplicateFunctionDef,
                format!("function '{name}' is already defined"),
                Span::default(),
            ),
            LowerError::Unsupported { reason, line } => Diagnostic::error(
                ErrorCode::UnsupportedConstruct,
                format!("unsupported construct: {reason}"),
                Span::point(line, 0),
            ),
        }
    }
}

impl From<BackendError> for Diagnostic {
    fn from(e: BackendError) -> Self {
        match e {
            BackendError::CommandNotFound { command } => Diagnostic::error(
                ErrorCode::CommandNotFound,
                format!("command not found: '{command}'"),
                Span::default(),
            )
            .with_help("check that the command is installed and on your PATH"),
            BackendError::Unsupported { reason } => Diagnostic::error(
                ErrorCode::UnsupportedHirConstruct,
                format!("unsupported HIR construct: {reason}"),
                Span::default(),
            ),
            BackendError::Io(e) => Diagnostic::error(
                ErrorCode::IoError,
                format!("I/O error during backend lowering: {e}"),
                Span::default(),
            ),
        }
    }
}

impl From<ExecError> for Diagnostic {
    fn from(e: ExecError) -> Self {
        match e {
            ExecError::CommandNotFound(cmd) => Diagnostic::error(
                ErrorCode::CommandNotFound,
                format!("command not found: '{cmd}'"),
                Span::default(),
            )
            .with_help("check that the command is installed and on your PATH"),
            ExecError::MinVersionNotMet { required, current } => Diagnostic::error(
                ErrorCode::MinVersionNotMet,
                format!("script requires Forge >= {required}, but this is {current}"),
                Span::default(),
            )
            .with_help("run 'forge self-update' to upgrade"),
            ExecError::PlatformNotSupported { declared, current } => Diagnostic::error(
                ErrorCode::PlatformNotSupported,
                format!("script declares platform '{declared}' and cannot run on {current}"),
                Span::default(),
            ),
            ExecError::RequiredEnvMissing { vars } => Diagnostic::error(
                ErrorCode::RequiredEnvMissing,
                format!("required environment variable(s) not set: {vars}"),
                Span::default(),
            ),
            ExecError::EnvFileNotFound { path } => Diagnostic::error(
                ErrorCode::EnvFileNotFound,
                format!("env file not found: '{path}'"),
                Span::default(),
            ),
            ExecError::Timeout { timeout } => Diagnostic::error(
                ErrorCode::Timeout,
                format!("script exceeded timeout of {timeout}"),
                Span::default(),
            ),
            ExecError::UndefinedVariable { name } => Diagnostic::error(
                ErrorCode::UndefinedVariableRuntime,
                format!("variable '{name}' is not defined"),
                Span::default(),
            ),
            ExecError::DivisionByZero => Diagnostic::error(
                ErrorCode::DivisionByZero,
                "division by zero",
                Span::default(),
            ),
            ExecError::IntegerOverflow => Diagnostic::error(
                ErrorCode::IntegerOverflow,
                "integer overflow",
                Span::default(),
            ),
            ExecError::TypeError { op, left, right } => Diagnostic::error(
                ErrorCode::TypeError,
                format!("type error: cannot apply '{op}' to {left} and {right}"),
                Span::default(),
            ),
            ExecError::Io(e) => Diagnostic::error(
                ErrorCode::IoError,
                format!("I/O error during execution: {e}"),
                Span::default(),
            ),
            ExecError::InvalidArgument(msg) => Diagnostic::error(
                ErrorCode::UnsupportedConstruct,
                format!("invalid argument: {msg}"),
                Span::default(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_unexpected_char_maps_to_e001() {
        let d = Diagnostic::from(LexError::UnexpectedChar {
            ch: '$',
            line: 3,
            col: 5,
        });
        assert_eq!(d.code, ErrorCode::UnexpectedChar);
        assert_eq!(d.span.line, 3);
        assert_eq!(d.span.col, 5);
    }

    #[test]
    fn lex_unterminated_string_maps_to_e002() {
        let d = Diagnostic::from(LexError::UnterminatedString { line: 1, col: 1 });
        assert_eq!(d.code, ErrorCode::UnterminatedString);
    }

    #[test]
    fn lex_invalid_number_maps_to_e003() {
        let d = Diagnostic::from(LexError::InvalidNumber {
            literal: "1_2_".to_string(),
            line: 2,
            col: 3,
        });
        assert_eq!(d.code, ErrorCode::InvalidNumber);
    }

    #[test]
    fn lex_unterminated_interpolation_maps_to_e004() {
        let d = Diagnostic::from(LexError::UnterminatedInterpolation { line: 1, col: 8 });
        assert_eq!(d.code, ErrorCode::UnterminatedInterpolation);
    }

    #[test]
    fn parse_unexpected_eof_maps_to_e011_with_default_span() {
        let d = Diagnostic::from(ParseError::UnexpectedEof {
            expected: "}".to_string(),
        });
        assert_eq!(d.code, ErrorCode::UnexpectedEof);
        assert_eq!(d.span, Span::default());
    }

    #[test]
    fn parse_directive_after_statement_has_help() {
        let d = Diagnostic::from(ParseError::DirectiveAfterStatement {
            key: "timeout".to_string(),
            line: 5,
            col: 0,
        });
        assert_eq!(d.code, ErrorCode::DirectiveAfterStatement);
        assert!(d.help.is_some());
    }

    #[test]
    fn lower_undefined_variable_has_help() {
        let d = Diagnostic::from(LowerError::UndefinedVariable {
            name: "counter".to_string(),
            line: 5,
        });
        assert_eq!(d.code, ErrorCode::UndefinedVariable);
        assert!(d.help.is_some());
    }

    #[test]
    fn lower_circular_import_maps_to_e021() {
        let d = Diagnostic::from(LowerError::CircularImport {
            path: "./a".to_string(),
        });
        assert_eq!(d.code, ErrorCode::CircularImport);
    }

    #[test]
    fn exec_division_by_zero_maps_to_e046() {
        let d = Diagnostic::from(ExecError::DivisionByZero);
        assert_eq!(d.code, ErrorCode::DivisionByZero);
    }

    #[test]
    fn exec_type_error_maps_to_e048() {
        let d = Diagnostic::from(ExecError::TypeError {
            op: "+".to_string(),
            left: "int".to_string(),
            right: "string".to_string(),
        });
        assert_eq!(d.code, ErrorCode::TypeError);
    }

    #[test]
    fn all_from_impls_produce_error_severity() {
        use crate::diagnostic::Severity;
        let cases = vec![
            Diagnostic::from(LexError::UnterminatedString { line: 1, col: 1 }),
            Diagnostic::from(ParseError::UnexpectedEof {
                expected: ";".to_string(),
            }),
            Diagnostic::from(LowerError::DuplicateFunctionDef {
                name: "foo".to_string(),
            }),
            Diagnostic::from(ExecError::DivisionByZero),
        ];
        for d in cases {
            assert_eq!(d.severity, Severity::Error);
        }
    }
}

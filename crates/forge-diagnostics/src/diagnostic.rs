use forge_types::Span;

use crate::code::ErrorCode;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

/// A structured error or warning emitted by a pipeline pass.
///
/// Every pass returns `Result<Output, Vec<Diagnostic>>`. The `Diagnostic`
/// carries enough information for the renderer to produce rustc-style output
/// with a source context line and a caret pointing to the problem.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: ErrorCode,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Create an error-severity diagnostic.
    pub fn error(code: ErrorCode, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            help: None,
            notes: Vec::new(),
        }
    }

    /// Create a warning-severity diagnostic.
    pub fn warning(code: ErrorCode, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span,
            help: None,
            notes: Vec::new(),
        }
    }

    /// Attach a suggested fix.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Attach an additional context note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_span() -> Span {
        Span::point(3, 5)
    }

    #[test]
    fn error_has_error_severity() {
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "unexpected '$'", dummy_span());
        assert_eq!(d.severity, Severity::Error);
    }

    #[test]
    fn warning_has_warning_severity() {
        let d = Diagnostic::warning(
            ErrorCode::DirectiveAfterStatement,
            "directive ignored",
            dummy_span(),
        );
        assert_eq!(d.severity, Severity::Warning);
    }

    #[test]
    fn with_help_sets_help() {
        let d = Diagnostic::error(ErrorCode::UndefinedVariable, "undefined 'x'", dummy_span())
            .with_help("declare with 'let x = ...'");
        assert_eq!(d.help.as_deref(), Some("declare with 'let x = ...'"));
    }

    #[test]
    fn with_note_appends() {
        let d = Diagnostic::error(ErrorCode::CircularImport, "cycle", dummy_span())
            .with_note("a → b")
            .with_note("b → a");
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.notes[0], "a → b");
    }

    #[test]
    fn no_help_by_default() {
        let d = Diagnostic::error(ErrorCode::UnexpectedEof, "unexpected EOF", Span::default());
        assert!(d.help.is_none());
    }

    #[test]
    fn no_notes_by_default() {
        let d = Diagnostic::error(ErrorCode::UnexpectedEof, "unexpected EOF", Span::default());
        assert!(d.notes.is_empty());
    }
}

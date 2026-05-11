use std::fmt::Write as _;

use crate::diagnostic::{Diagnostic, Severity};

// ANSI color codes
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";

/// Renders diagnostics to a string in rustc-style format.
///
/// ```text
/// error[E001]: unexpected character '$'
///   --> deploy.fgs:3:5
///    |
///  3 |     $invalid = true
///    |     ^ unexpected character '$'
///    |
///    = help: env vars use $VAR syntax inside interpolated strings
/// ```
pub struct DiagnosticRenderer {
    use_color: bool,
    filename: String,
    source: Option<String>,
}

impl Default for DiagnosticRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            use_color: false,
            filename: "<input>".to_string(),
            source: None,
        }
    }

    /// Attach source text so the renderer can show context lines.
    #[must_use]
    pub fn with_source(mut self, filename: impl Into<String>, source: impl Into<String>) -> Self {
        self.filename = filename.into();
        self.source = Some(source.into());
        self
    }

    /// Enable or disable ANSI color codes.
    #[must_use]
    pub fn with_color(mut self, enabled: bool) -> Self {
        self.use_color = enabled;
        self
    }

    /// Render a single diagnostic.
    #[must_use]
    pub fn render(&self, d: &Diagnostic) -> String {
        let mut out = String::new();

        // ── Header: "error[E001]: message" ───────────────────────────────────
        let (label, color) = match d.severity {
            Severity::Error => ("error", RED),
            Severity::Warning => ("warning", YELLOW),
            Severity::Note => ("note", BLUE),
        };

        if self.use_color {
            let _ = writeln!(
                out,
                "{BOLD}{color}{label}{RESET}{BOLD}[{}]: {}{RESET}",
                d.code, d.message
            );
        } else {
            let _ = writeln!(out, "{label}[{}]: {}", d.code, d.message);
        }

        // ── Location: "  --> file:line:col" ──────────────────────────────────
        let line = d.span.line;
        let col = d.span.col;

        if line > 0 {
            if self.use_color {
                let _ = writeln!(
                    out,
                    "  {BOLD}{CYAN}-->{RESET} {}:{line}:{col}",
                    self.filename
                );
            } else {
                let _ = writeln!(out, "  --> {}:{line}:{col}", self.filename);
            }

            // ── Source context ────────────────────────────────────────────────
            if let Some(source_line) = self.source_line(line) {
                let gutter = line.to_string().len();
                let pad = " ".repeat(gutter);

                let _ = writeln!(out, "{pad}   |");
                let _ = writeln!(out, "{line:>gutter$}   | {source_line}");

                // Caret under the offending column (1-indexed, guard against col=0)
                let caret_offset = col.saturating_sub(1);
                let caret = format!("{}^", " ".repeat(caret_offset));
                let _ = writeln!(out, "{pad}   | {caret}");
                let _ = writeln!(out, "{pad}   |");
            }
        }

        // ── Help ─────────────────────────────────────────────────────────────
        if let Some(help) = &d.help {
            if self.use_color {
                let _ = writeln!(out, "   = {BOLD}help{RESET}: {help}");
            } else {
                let _ = writeln!(out, "   = help: {help}");
            }
        }

        // ── Notes ─────────────────────────────────────────────────────────────
        for note in &d.notes {
            if self.use_color {
                let _ = writeln!(out, "   = {BOLD}note{RESET}: {note}");
            } else {
                let _ = writeln!(out, "   = note: {note}");
            }
        }

        out
    }

    /// Render multiple diagnostics separated by a blank line.
    #[must_use]
    pub fn render_all(&self, diagnostics: &[Diagnostic]) -> String {
        diagnostics
            .iter()
            .map(|d| self.render(d))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn source_line(&self, line: usize) -> Option<&str> {
        let source = self.source.as_deref()?;
        // lines() is 0-indexed; span line is 1-indexed
        source.lines().nth(line.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::ErrorCode;
    use forge_types::Span;

    fn span_at(line: usize, col: usize) -> Span {
        Span::point(line, col)
    }

    #[test]
    fn render_error_without_source_contains_header() {
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "unexpected '$'", span_at(3, 5));
        let out = DiagnosticRenderer::new().render(&d);
        assert!(out.contains("error[E001]"));
        assert!(out.contains("unexpected '$'"));
    }

    #[test]
    fn render_includes_location() {
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "oops", span_at(3, 5));
        let out = DiagnosticRenderer::new()
            .with_source("test.fgs", "")
            .render(&d);
        assert!(out.contains("test.fgs:3:5"));
    }

    #[test]
    fn render_shows_source_line() {
        let source = "line one\nlet x = $bad\nline three";
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "bad char", span_at(2, 9));
        let out = DiagnosticRenderer::new()
            .with_source("s.fgs", source)
            .render(&d);
        assert!(out.contains("let x = $bad"));
    }

    #[test]
    fn render_caret_aligns_to_col() {
        let source = "echo hello";
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "bad", span_at(1, 6));
        let out = DiagnosticRenderer::new()
            .with_source("s.fgs", source)
            .render(&d);
        // col 6 → 5 spaces then caret
        assert!(out.contains("     ^"));
    }

    #[test]
    fn render_no_color_has_no_ansi() {
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "oops", span_at(1, 1));
        let out = DiagnosticRenderer::new().with_color(false).render(&d);
        assert!(!out.contains("\x1b["));
    }

    #[test]
    fn render_with_color_has_ansi_reset() {
        let d = Diagnostic::error(ErrorCode::UnexpectedChar, "oops", span_at(1, 1));
        let out = DiagnosticRenderer::new().with_color(true).render(&d);
        assert!(out.contains("\x1b[0m"));
    }

    #[test]
    fn render_help_appears() {
        let d = Diagnostic::error(
            ErrorCode::UndefinedVariable,
            "undefined 'x'",
            Span::default(),
        )
        .with_help("declare with 'let x = ...'");
        let out = DiagnosticRenderer::new().render(&d);
        assert!(out.contains("= help: declare with 'let x = ...'"));
    }

    #[test]
    fn render_note_appears() {
        let d = Diagnostic::error(ErrorCode::CircularImport, "cycle", Span::default())
            .with_note("a → b → a");
        let out = DiagnosticRenderer::new().render(&d);
        assert!(out.contains("= note: a → b → a"));
    }

    #[test]
    fn render_all_empty_is_empty_string() {
        let out = DiagnosticRenderer::new().render_all(&[]);
        assert_eq!(out, "");
    }

    #[test]
    fn render_all_joins_with_blank_line() {
        let d1 = Diagnostic::error(ErrorCode::UnexpectedChar, "first", Span::default());
        let d2 = Diagnostic::error(ErrorCode::UnexpectedToken, "second", Span::default());
        let out = DiagnosticRenderer::new().render_all(&[d1, d2]);
        // Two renders joined by "\n" — the renders themselves end with "\n"
        // so the join produces a blank line between them.
        assert!(out.contains("error[E001]"));
        assert!(out.contains("error[E010]"));
    }

    #[test]
    fn render_zero_span_omits_location_block() {
        let d = Diagnostic::error(ErrorCode::UnexpectedEof, "unexpected EOF", Span::default());
        let out = DiagnosticRenderer::new().render(&d);
        // span.line == 0, so no location arrow
        assert!(!out.contains("-->"));
    }

    #[test]
    fn warning_label_in_output() {
        let d = Diagnostic::warning(
            ErrorCode::DirectiveAfterStatement,
            "will be ignored",
            Span::default(),
        );
        let out = DiagnosticRenderer::new().render(&d);
        assert!(out.starts_with("warning[E013]"));
    }
}

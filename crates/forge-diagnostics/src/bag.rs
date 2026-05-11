use crate::diagnostic::{Diagnostic, Severity};

/// Collects diagnostics emitted during a pipeline pass.
///
/// Passes accumulate errors here rather than failing fast so that all errors
/// in a single pass are reported together. Between passes, call
/// `into_result()` to halt the pipeline if any errors were collected.
#[derive(Debug, Default)]
pub struct DiagnosticBag {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    pub fn extend(&mut self, iter: impl IntoIterator<Item = Diagnostic>) {
        self.diagnostics.extend(iter);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Consume the bag. Returns `Ok(())` if no errors were collected,
    /// `Err(Vec<Diagnostic>)` otherwise. Warnings are included in the `Err`
    /// vec alongside errors so callers can render them all at once.
    ///
    /// # Errors
    /// Returns `Err` when any `Severity::Error` diagnostic was pushed.
    pub fn into_result(self) -> Result<(), Vec<Diagnostic>> {
        if self.has_errors() {
            Err(self.diagnostics)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::ErrorCode;
    use forge_types::Span;

    fn err() -> Diagnostic {
        Diagnostic::error(ErrorCode::UnexpectedChar, "oops", Span::default())
    }

    fn warn() -> Diagnostic {
        Diagnostic::warning(
            ErrorCode::DirectiveAfterStatement,
            "heads up",
            Span::default(),
        )
    }

    #[test]
    fn empty_bag_has_no_errors() {
        assert!(!DiagnosticBag::new().has_errors());
    }

    #[test]
    fn empty_bag_is_empty() {
        assert!(DiagnosticBag::new().is_empty());
    }

    #[test]
    fn bag_with_error_has_errors() {
        let mut bag = DiagnosticBag::new();
        bag.push(err());
        assert!(bag.has_errors());
    }

    #[test]
    fn bag_with_only_warning_has_no_errors() {
        let mut bag = DiagnosticBag::new();
        bag.push(warn());
        assert!(!bag.has_errors());
    }

    #[test]
    fn into_result_empty_is_ok() {
        assert!(DiagnosticBag::new().into_result().is_ok());
    }

    #[test]
    fn into_result_with_error_is_err() {
        let mut bag = DiagnosticBag::new();
        bag.push(err());
        assert!(bag.into_result().is_err());
    }

    #[test]
    fn into_result_warnings_only_is_ok() {
        let mut bag = DiagnosticBag::new();
        bag.push(warn());
        assert!(bag.into_result().is_ok());
    }

    #[test]
    fn errors_iterator_skips_warnings() {
        let mut bag = DiagnosticBag::new();
        bag.push(err());
        bag.push(warn());
        assert_eq!(bag.errors().count(), 1);
    }

    #[test]
    fn warnings_iterator_skips_errors() {
        let mut bag = DiagnosticBag::new();
        bag.push(err());
        bag.push(warn());
        assert_eq!(bag.warnings().count(), 1);
    }

    #[test]
    fn extend_adds_all_items() {
        let mut bag = DiagnosticBag::new();
        bag.extend([err(), warn(), err()]);
        assert_eq!(bag.into_vec().len(), 3);
    }
}

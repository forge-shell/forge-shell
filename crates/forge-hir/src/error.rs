use thiserror::Error;

/// Errors that can occur during AST→HIR lowering.
#[derive(Debug, Error)]
pub enum LowerError {
    /// A name was used that was never declared in any accessible scope.
    /// Env vars (`$VAR`) are exempt — they bypass scope checking.
    #[error("undefined variable '{name}' at line {line}")]
    UndefinedVariable { name: String, line: usize },

    /// An import creates a cycle: A imports B which imports A.
    #[error("circular import: '{path}' is already being imported")]
    CircularImport { path: String },

    /// A function was defined more than once in the same scope.
    #[error("function '{name}' is already defined")]
    DuplicateFunctionDef { name: String },

    /// An expression or statement form is not yet supported in the lowerer.
    #[error("unsupported construct at line {line}: {reason}")]
    Unsupported { reason: String, line: usize },
}

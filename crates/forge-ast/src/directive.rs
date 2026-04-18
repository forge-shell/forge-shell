use forge_types::Span;

/// A parsed directive from a `#!` line.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Directive {
    pub kind: DirectiveKind,
    pub span: Span,
}

/// The structured content of a directive.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum DirectiveKind {
    /// `#!/path/to/interpreter` — Unix shebang. Ignored by the executor on all platforms.
    UnixShebang(String),

    /// `#!forge:description = "..."` — human-readable script description.
    Description(String),

    /// `#!forge:author = "..."` — script author name.
    Author(String),

    /// `#!forge:min-version = "0.3.0"` — minimum Forge Shell version.
    /// Stored as a raw string; full semver validation happens in the parser.
    MinVersion(String),

    /// `#!forge:platform = "unix"` — supported platforms.
    Platform(Vec<Platform>),

    /// `#!forge:overflow = "saturate"` — integer overflow behaviour.
    Overflow(OverflowMode),

    /// `#!forge:strict = true` — fail on first non-zero exit.
    Strict(bool),

    /// `#!forge:timeout = "5m"` — maximum wall-clock execution time.
    /// Stored as the raw string; duration parsing happens in the executor.
    Timeout(String),

    /// `#!forge:jobs = "4"` or `#!forge:jobs = "auto"` — max parallel jobs.
    Jobs(JobLimit),

    /// `#!forge:env-file = ".env"` — env file to load before execution.
    EnvFile(String),

    /// `#!forge:require-env = "VAR1,VAR2"` — required environment variables.
    RequireEnv(Vec<String>),

    /// `#!forge:override = "ls"` — built-in command this script overrides.
    Override(String),

    /// An unrecognised `#!forge:` key. Preserved for diagnostics; produces a compile-time warning.
    Unknown { key: String, value: String },
}

/// Supported platform targets for the `platform` directive.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    All,
    /// Linux + macOS.
    Unix,
    Linux,
    MacOs,
    Windows,
}

/// Integer overflow behaviour.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverflowMode {
    Panic,
    Saturate,
    Wrap,
}

/// Maximum parallel job limit.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobLimit {
    Count(u32),
    Auto,
}

#[cfg(test)]
mod directive_tests {
    use super::*;

    fn make_span() -> Span {
        Span {
            start: 0,
            end: 1,
            line: 1,
            col: 1,
        }
    }

    #[test]
    fn test_directive_unix_shebang() {
        let d = Directive {
            kind: DirectiveKind::UnixShebang("/usr/bin/env forge".to_string()),
            span: make_span(),
        };
        assert!(matches!(d.kind, DirectiveKind::UnixShebang(_)));
    }

    #[test]
    fn test_directive_overflow_variants() {
        assert_eq!(OverflowMode::Panic, OverflowMode::Panic);
        assert_ne!(OverflowMode::Panic, OverflowMode::Saturate);
    }

    #[test]
    fn test_directive_platform_variants() {
        let platforms = [Platform::Linux, Platform::MacOs];
        assert_eq!(platforms.len(), 2);
    }

    #[test]
    fn test_job_limit_auto() {
        assert_eq!(JobLimit::Auto, JobLimit::Auto);
        assert_ne!(JobLimit::Auto, JobLimit::Count(4));
    }

    #[test]
    fn test_program_with_directives() {
        use crate::program::Program;

        let prog = Program {
            directives: vec![Directive {
                kind: DirectiveKind::Strict(true),
                span: make_span(),
            }],
            stmts: vec![],
        };
        assert_eq!(prog.directives.len(), 1);
        assert_eq!(prog.stmts.len(), 0);
    }

    #[test]
    fn test_unknown_directive_preserved() {
        let d = Directive {
            kind: DirectiveKind::Unknown {
                key: "future-key".to_string(),
                value: "some-value".to_string(),
            },
            span: make_span(),
        };
        assert!(matches!(d.kind, DirectiveKind::Unknown { .. }));
    }
}

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

/// A byte-range and line/column position in the source text.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Span {
    /// Byte offset of the first character (inclusive).
    pub start: usize,
    /// Byte offset after the last character (exclusive).
    pub end: usize,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column number.
    pub col: usize,
}

impl Span {
    #[must_use]
    pub fn new(start: usize, end: usize, line: usize, col: usize) -> Self {
        Self {
            start,
            end,
            line,
            col,
        }
    }

    /// Create a zero-width span at a given position.
    #[must_use]
    pub fn point(line: usize, col: usize) -> Self {
        Self {
            start: 0,
            end: 0,
            line,
            col,
        }
    }

    /// Merge two spans into one covering both.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            line: self.line.min(other.line),
            col: self.col,
        }
    }
}

/// The canonical list of all `ForgeScript` built-in command names.
/// Both `forge-backend` and `forge-exec` import from here.
/// Add new built-ins here first — the rest of the codebase follows.
pub const BUILTIN_NAMES: &[&str] = &[
    // File System (13)
    "ls", "tree", "cp", "mv", "rm", "mkdir", "rmdir", "touch", "find", "stat", "du", "df", "hash",
    // Text & Streams (12)
    "cat", "echo", "grep", "diff", "head", "tail", "sort", "uniq", "wc", "jq", "yq", "tq",
    // Environment & Process (7)
    "env", "set", "unset", "which", "exit", "pwd", "cd",
];

/// Returns true if the given name is a `ForgeScript` built-in command.
#[must_use]
pub fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

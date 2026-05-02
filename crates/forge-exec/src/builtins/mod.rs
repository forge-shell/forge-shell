#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod env;
pub mod fs;
pub mod text;
pub mod utils;

// Re-export filter at the builtins level for existing `crate::builtins::filter` consumers.
pub use text::filter;

use crate::{ExecError, ShellContext};

/// A built-in shell command - runs in-process, never spawns as an OS process.
pub trait BuiltinCommand: Send + Sync {
    /// The command name as used in scripts - e.g. `"echo"`.
    fn name(&self) -> &'static str;

    /// Execute the command.
    ///
    /// # Errors
    ///
    /// Returns `ExecError` if the command fails or arguments are invalid.
    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError>;
}

/// Registry of all built-in commands.
///
/// Uses `Vec` not `HashMap` — at shell scale (< 40 built-ins), linear search
/// is faster than hash lookup due to cache locality. Order of registration
/// matches `forge_types::BUILTIN_NAMES`.
pub struct BuiltinRegistry {
    commands: Vec<Box<dyn BuiltinCommand>>,
}

impl BuiltinRegistry {
    /// Create a registry pre-populated with all built-in commands.
    #[must_use]
    pub fn new() -> Self {
        let mut r = Self {
            commands: Vec::new(),
        };

        // File system (13)
        r.register(Box::new(fs::Ls));
        r.register(Box::new(fs::Tree));
        r.register(Box::new(fs::Cp));
        r.register(Box::new(fs::Mv));
        r.register(Box::new(fs::Rm));
        r.register(Box::new(fs::Mkdir));
        r.register(Box::new(fs::Rmdir));
        r.register(Box::new(fs::Touch));
        r.register(Box::new(fs::Find));
        r.register(Box::new(fs::Stat));
        r.register(Box::new(fs::Du));
        r.register(Box::new(fs::Df));
        r.register(Box::new(fs::Hash));

        // Text & streams (12)
        r.register(Box::new(text::Cat));
        r.register(Box::new(text::Echo));
        r.register(Box::new(text::Grep));
        r.register(Box::new(text::Diff));
        r.register(Box::new(text::Head));
        r.register(Box::new(text::Tail));
        r.register(Box::new(text::Sort));
        r.register(Box::new(text::Uniq));
        r.register(Box::new(text::Wc));
        r.register(Box::new(text::Jq));
        r.register(Box::new(text::Yq));
        r.register(Box::new(text::Tq));

        // Environment & process (7)
        r.register(Box::new(env::Env));
        r.register(Box::new(env::Set));
        r.register(Box::new(env::Unset));
        r.register(Box::new(env::Which));
        r.register(Box::new(env::Exit));
        r.register(Box::new(env::Pwd));
        r.register(Box::new(env::Cd));

        r
    }

    /// Register a built-in command.
    pub fn register(&mut self, cmd: Box<dyn BuiltinCommand>) {
        self.commands.push(cmd);
    }

    /// Look up a built-in by name. Returns `None` if not registered.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn BuiltinCommand> {
        self.commands
            .iter()
            .find(|c| c.name() == name)
            .map(Box::as_ref)
    }

    /// Returns true if the name matches a registered built-in.
    #[must_use]
    pub fn is_builtin(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// All registered command names — used for tab completion.
    #[must_use]
    #[allow(dead_code)]
    pub fn names(&self) -> Vec<&'static str> {
        self.commands.iter().map(|c| c.name()).collect()
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;

    #[test]
    fn test_registry_contains_all_builtins() {
        let registry = BuiltinRegistry::new();
        for name in forge_types::BUILTIN_NAMES {
            assert!(registry.is_builtin(name), "missing from registry: {name}");
        }
    }

    #[test]
    fn test_registry_names_match_builtin_names() {
        let registry = BuiltinRegistry::new();
        let mut names = registry.names();
        names.sort_unstable();
        let mut expected: Vec<&str> = forge_types::BUILTIN_NAMES.to_vec();
        expected.sort_unstable();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_registry_unknown_returns_none() {
        let registry = BuiltinRegistry::new();
        assert!(registry.get("not_a_builtin").is_none());
    }

    #[test]
    fn test_registry_dispatches_echo() {
        let registry = BuiltinRegistry::new();
        let cmd = registry.get("echo").unwrap();
        assert_eq!(
            cmd.run(&["hello".to_string()], &mut ShellContext::new())
                .unwrap(),
            0
        );
    }
}

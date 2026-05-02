#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// TODO

use crate::error::BackendError;
use crate::plan::ExecutionPlan;
use forge_hir::HirProgram;

pub mod error;
pub mod lower;
pub mod plan;
#[cfg(not(windows))]
pub mod unix;

#[cfg(windows)]
pub mod windows;

/// The cross-platform lowering interface.
///
/// Implementations translate `HirProgram` nodes into `ExecutionPlan` ops
/// using OS-specific knowledge such as path resolution, env var expansion, etc.
///
/// All implementations must be `Send + Sync` because the executor may run
/// plans on worker threads in the future milestones.
pub trait PlatformBackend: Send + Sync {
    /// Lower a complete HIR program into an execution plan.
    ///
    /// # Errors
    /// Returns `BackendError` if any HIR construct cannot be lowered.
    fn lower(&self, program: &HirProgram) -> Result<ExecutionPlan, BackendError>;

    /// Resolve a bare command name to its full executable path.
    ///
    /// Returns `None` if the command is not found on PATH.
    /// Built-in commands bypass this. They never need path resolution.
    ///
    /// # Errors
    /// Returns `BackendError` if the command cannot be resolved.
    fn resolve_command(&self, name: &str) -> Result<String, BackendError>;

    /// Expand `~` and `$VAR` references in a path string.
    fn expand_path(&self, path: &str, env: &std::collections::HashMap<String, String>) -> String;

    /// Return the PATH separator for this platform.
    /// `:` on Unix, `;` on Windows.
    fn path_separator(&self) -> &'static str;

    /// Return the name of the current platform for error messages.
    fn platform_name(&self) -> &'static str;
}

/// Select the correct backend for the current compilation target.
/// Called once at process startup.
#[must_use]
pub fn platform_backend() -> Box<dyn PlatformBackend> {
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsBackend::new());

    #[cfg(not(target_os = "windows"))]
    return Box::new(unix::UnixBackend::new());
}

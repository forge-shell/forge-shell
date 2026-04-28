use crate::{PlatformBackend, error::BackendError, plan::ExecutionPlan};
use forge_hir::HirProgram;
use std::collections::HashMap;

pub struct WindowsBackend;

impl WindowsBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for WindowsBackend {
    fn lower(&self, _program: &HirProgram) -> Result<ExecutionPlan, BackendError> {
        Ok(ExecutionPlan::empty())
    }

    fn resolve_command(&self, _name: &str) -> Result<String, BackendError> {
        Ok(String::new())
    }

    fn expand_path(&self, path: &str, _env: &HashMap<String, String>) -> String {
        path.to_string()
    }

    fn path_separator(&self) -> &'static str {
        ";"
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }
}

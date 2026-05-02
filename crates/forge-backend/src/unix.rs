use crate::lower::HirLowerer;
use crate::{PlatformBackend, error::BackendError, plan::ExecutionPlan};
use forge_hir::HirProgram;
use forge_types::BUILTIN_NAMES;
use std::collections::HashMap;

pub struct UnixBackend;

/// Platform backend for Linux and macOS.
impl UnixBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnixBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformBackend for UnixBackend {
    fn lower(&self, program: &HirProgram) -> Result<ExecutionPlan, BackendError> {
        HirLowerer::new(self).lower_program(program)
    }

    fn resolve_command(&self, name: &str) -> Result<String, BackendError> {
        // Built-in commands do not need path resolution
        if is_builtin(name) {
            return Err(BackendError::CommandNotFound {
                command: name.to_string(),
            });
        }

        let path = std::path::Path::new(name);
        if path.is_absolute() && path.is_file() {
            return Ok(name.to_string());
        }

        let path_var = std::env::var("PATH").unwrap_or_default();
        for dir in path_var.split(self.path_separator()) {
            let candidate = std::path::Path::new(dir).join(name);
            if candidate.is_file() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }

        Err(BackendError::CommandNotFound {
            command: name.to_string(),
        })
    }

    fn expand_path(&self, path: &str, env: &HashMap<String, String>) -> String {
        // Check ~/path first — more specific match must come before bare ~
        if let Some(rest) = path.strip_prefix("~/") {
            if let Some(home) = env.get("HOME") {
                return format!("{home}/{rest}");
            }
        }

        // Bare ~ with nothing after it
        if path == "~" {
            return env.get("HOME").cloned().unwrap_or_else(|| "~".to_string());
        }

        expand_env_vars(path, env)
    }

    fn path_separator(&self) -> &'static str {
        ":"
    }

    fn platform_name(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        }
    }
}

fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

fn expand_env_vars(path: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            i += 1;
            if chars[i] == '{' {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                result.push_str(env.get(&var_name).map_or("", std::string::String::as_str));
                if i < chars.len() {
                    i += 1;
                }
            } else {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let var_name: String = chars[start..i].iter().collect();
                result.push_str(env.get(&var_name).map_or("", std::string::String::as_str));
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tilde_expansion() {
        let backend = UnixBackend::new();
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/home/ajitem".to_string());

        assert_eq!(
            backend.expand_path("~/projects", &env),
            "/home/ajitem/projects"
        );
        assert_eq!(backend.expand_path("~", &env), "/home/ajitem");
        assert_eq!(backend.expand_path("/absolute", &env), "/absolute");
    }

    #[test]
    fn test_env_var_expansion() {
        let backend = UnixBackend::new();
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/home/ajitem".to_string());
        env.insert("USER".to_string(), "ajitem".to_string());

        assert_eq!(backend.expand_path("$HOME/docs", &env), "/home/ajitem/docs");
        assert_eq!(backend.expand_path("${USER}/file", &env), "ajitem/file");
    }

    #[test]
    fn test_path_separator() {
        assert_eq!(UnixBackend::new().path_separator(), ":");
    }

    #[test]
    fn test_builtin_returns_error() {
        let backend = UnixBackend::new();
        // Built-ins return Err — handled by forge-exec directly
        assert!(backend.resolve_command("echo").is_err());
        assert!(backend.resolve_command("cd").is_err());
    }

    #[test]
    fn test_missing_command_returns_err() {
        let backend = UnixBackend::new();
        assert!(matches!(
            backend.resolve_command("this_command_does_not_exist_xyz"),
            Err(BackendError::CommandNotFound { .. })
        ));
    }

    #[test]
    fn test_platform_name() {
        let name = UnixBackend::new().platform_name();
        assert!(
            name == "linux" || name == "macos",
            "unexpected platform name: {name}"
        );
    }
}

use crate::lower::HirLowerer;
use crate::{PlatformBackend, error::BackendError, plan::ExecutionPlan};
use forge_hir::HirProgram;
use forge_types::BUILTIN_NAMES;
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
    fn lower(&self, program: &HirProgram) -> Result<ExecutionPlan, BackendError> {
        HirLowerer::new(self).lower_program(program)
    }

    fn resolve_command(&self, name: &str) -> Result<String, BackendError> {
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
        let extensions = ["", ".exe", ".bat", ".cmd"];

        for dir in path_var.split(self.path_separator()) {
            for ext in &extensions {
                let candidate = std::path::Path::new(dir).join(format!("{name}{ext}"));
                if candidate.is_file() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }

        Err(BackendError::CommandNotFound {
            command: name.to_string(),
        })
    }

    fn expand_path(&self, path: &str, env: &HashMap<String, String>) -> String {
        if path == "~" {
            return env
                .get("USERPROFILE")
                .cloned()
                .unwrap_or_else(|| "~".to_string());
        }
        if path.starts_with("~/") || path.starts_with("~\\") {
            if let Some(home) = env.get("USERPROFILE") {
                return format!("{}\\{}", home, &path[2..]);
            }
        }
        let after_dollar = expand_dollar_vars(path, env);
        expand_percent_vars(&after_dollar, env)
    }

    fn path_separator(&self) -> &'static str {
        ";"
    }

    fn platform_name(&self) -> &'static str {
        "windows"
    }
}

fn is_builtin(name: &str) -> bool {
    BUILTIN_NAMES.contains(&name)
}

fn expand_dollar_vars(path: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(path.len());
    let chars: Vec<char> = path.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '$' && index + 1 < chars.len() {
            index += 1;

            if chars[index] == '{' {
                expand_braced_dollar_var(&chars, env, &mut result, &mut index);
            } else {
                expand_unbraced_dollar_var(&chars, env, &mut result, &mut index);
            }
        } else {
            result.push(chars[index]);
            index += 1;
        }
    }

    result
}

fn expand_braced_dollar_var(
    chars: &[char],
    env: &HashMap<String, String>,
    result: &mut String,
    index: &mut usize,
) {
    *index += 1;
    let name_start = *index;

    while *index < chars.len() && chars[*index] != '}' {
        *index += 1;
    }

    let var_name: String = chars[name_start..*index].iter().collect();
    result.push_str(lookup_env_case_insensitive(env, &var_name));

    if *index < chars.len() {
        *index += 1;
    }
}

fn expand_unbraced_dollar_var(
    chars: &[char],
    env: &HashMap<String, String>,
    result: &mut String,
    index: &mut usize,
) {
    let name_start = *index;

    while *index < chars.len() && is_env_var_name_char(chars[*index]) {
        *index += 1;
    }

    let var_name: String = chars[name_start..*index].iter().collect();
    result.push_str(lookup_env_case_insensitive(env, &var_name));
}

fn is_env_var_name_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn lookup_env_case_insensitive<'env>(
    env: &'env HashMap<String, String>,
    var_name: &str,
) -> &'env str {
    env.iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(var_name))
        .map_or("", |(_, value)| value.as_str())
}

fn expand_percent_vars(s: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            let mut var_name = String::new();
            let mut closed = false;
            for inner in chars.by_ref() {
                if inner == '%' {
                    closed = true;
                    break;
                }
                var_name.push(inner);
            }
            if closed && !var_name.is_empty() {
                let value = env
                    .iter()
                    .find(|(k, _)| k.to_lowercase() == var_name.to_lowercase())
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("");
                result.push_str(value);
            } else {
                result.push('%');
                result.push_str(&var_name);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_separator() {
        assert_eq!(WindowsBackend::new().path_separator(), ";");
    }

    #[test]
    fn test_tilde_expansion_windows() {
        let backend = WindowsBackend::new();
        let mut env = HashMap::new();
        env.insert("USERPROFILE".to_string(), r"C:\Users\ajitem".to_string());
        assert_eq!(
            backend.expand_path("~/projects", &env),
            r"C:\Users\ajitem\projects"
        );
    }

    #[test]
    fn test_percent_var_expansion() {
        let mut env = HashMap::new();
        env.insert("USERPROFILE".to_string(), r"C:\Users\ajitem".to_string());
        assert_eq!(
            expand_percent_vars(r"%USERPROFILE%\docs", &env),
            r"C:\Users\ajitem\docs"
        );
    }

    #[test]
    fn test_builtin_returns_error() {
        let backend = WindowsBackend::new();
        assert!(backend.resolve_command("echo").is_err());
    }

    #[test]
    fn test_platform_name() {
        assert_eq!(WindowsBackend::new().platform_name(), "windows");
    }
}

use forge_ast::OverflowMode;
use forge_backend::plan::Value;
use std::collections::HashMap;
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct ShellContext {
    /// Environment variables - inherited by child processes.
    pub env: HashMap<String, String>,
    /// `ForgeScript` variables - NOT exposed to child processes.
    pub vars: HashMap<String, Value>,
    /// Current working directory.
    pub cwd: std::path::PathBuf,
    /// Exit code of the most recently completed command.
    pub last_exit: i32,
    /// Integer overflow behaviour - set by `#!forge:overflow`.
    pub overflow_mode: OverflowMode,
    /// Fail on first non-zero exit - set by `#!forge:strict`.
    pub strict_mode: bool,
    /// Maximum parallel jobs - set by `#!forge:jobs`.
    pub max_jobs: u32,
    /// Execution timeout - set by `#!forge:timeout`.
    pub timeout: Option<std::time::Duration>,
    /// Marker that prevents `ShellContext` from being sent across threads.
    /// `ShellContext` is always owned by the main thread.
    _not_send: PhantomData<*const ()>,
}

impl ShellContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            env: std::env::vars().collect(),
            vars: HashMap::new(),
            cwd: std::env::current_dir().unwrap_or_default(),
            last_exit: 0,
            overflow_mode: OverflowMode::Panic,
            strict_mode: false,
            max_jobs: std::thread::available_parallelism().map_or(1, |n| {
                #[allow(clippy::cast_possible_truncation)]
                let jobs = n.get() as u32;
                jobs
            }),
            timeout: None,
            _not_send: PhantomData,
        }
    }

    /// Resole a `Value`, expanding `VarRef` and `EnvRef` at runtime.
    #[must_use]
    pub fn resolve(&self, val: &Value) -> Value {
        match val {
            Value::VarRef(name) => self.vars.get(name).cloned().unwrap_or(Value::Null),
            Value::EnvRef(name) => self
                .env
                .get(name)
                .map_or(Value::Null, |s| Value::Str(s.clone())),
            other => other.clone(),
        }
    }

    #[must_use]
    pub fn resolve_to_string(&self, val: &Value) -> String {
        self.resolve(val).to_string()
    }

    pub fn set_var(&mut self, name: &str, value: Value) {
        self.vars.insert(name.to_string(), value);
    }

    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());

        // SAFETY: ShellContext is always on the main thread.
        // No other Rust thread reads or writes the process environment.
        // Child processes are separate OS processes, not Rust threads.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(key, value);
        }
    }

    pub fn remove_env(&mut self, key: &str) {
        self.env.remove(key);
        // SAFETY: same invariant as set_env — main thread only.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(key);
        }
    }
}

impl Default for ShellContext {
    fn default() -> Self {
        Self::new()
    }
}

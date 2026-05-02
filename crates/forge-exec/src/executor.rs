use crate::builtins::BuiltinRegistry;
use crate::{context::ShellContext, error::ExecError};
use forge_ast::{Directive, DirectiveKind, JobLimit, OverflowMode, Platform};
use forge_backend::plan::{BinOpKind, ExecutionPlan, Op, StdioConfig, UnaryOpKind, Value};
use std::process::{Command, Stdio};

pub struct Executor {
    pub context: ShellContext,
    registry: BuiltinRegistry,
}

impl Executor {
    #[must_use]
    pub fn new(context: ShellContext) -> Self {
        Self {
            context,
            registry: BuiltinRegistry::new(),
        }
    }

    /// Execute a complete plan. Returns the final exit code.
    ///
    /// # Errors
    ///
    /// Returns `ExecError` if any operation fails.
    pub fn run(&mut self, plan: &ExecutionPlan) -> Result<i32, ExecError> {
        for op in &plan.ops {
            self.execute_op(op)?;
        }
        Ok(self.context.last_exit)
    }

    /// Enforce all directives before running any ops.
    /// Call this before `run()` when executing a script.
    ///
    /// # Errors
    ///
    /// Returns `ExecError` if any hard constraint is violated.
    pub fn enforce_directives(&mut self, directives: &[Directive]) -> Result<(), ExecError> {
        for directive in directives {
            match &directive.kind {
                DirectiveKind::UnixShebang(_)
                | DirectiveKind::Description(_)
                | DirectiveKind::Author(_)
                | DirectiveKind::Unknown { .. }
                | DirectiveKind::Override(_) => {}

                DirectiveKind::MinVersion(required) => {
                    let current = env!("CARGO_PKG_VERSION");
                    if !version_satisfies(current, required) {
                        return Err(ExecError::MinVersionNotMet {
                            required: required.clone(),
                            current: current.to_string(),
                        });
                    }
                }

                DirectiveKind::Platform(platforms) => {
                    let current = current_platform();
                    let supported =
                        platforms.contains(&Platform::All) || platforms.contains(&current);
                    if !supported {
                        return Err(ExecError::PlatformNotSupported {
                            declared: platforms
                                .iter()
                                .map(|p| format!("{p:?}"))
                                .collect::<Vec<_>>()
                                .join(", "),
                            current: format!("{current:?}"),
                        });
                    }
                }

                DirectiveKind::Overflow(mode) => {
                    self.context.overflow_mode = mode.clone();
                }

                DirectiveKind::Strict(enabled) => {
                    self.context.strict_mode = *enabled;
                }

                DirectiveKind::Timeout(duration) => {
                    self.context.timeout = Some(parse_duration(duration));
                }

                DirectiveKind::Jobs(limit) => {
                    self.context.max_jobs = match limit {
                        JobLimit::Auto => std::thread::available_parallelism().map_or(1, |n| {
                            #[allow(clippy::cast_possible_truncation)]
                            let jobs = n.get() as u32;
                            jobs
                        }),
                        JobLimit::Count(n) => *n,
                    };
                }

                DirectiveKind::EnvFile(path) => {
                    self.load_env_file(path)?;
                }

                DirectiveKind::RequireEnv(vars) => {
                    let missing: Vec<&str> = vars
                        .iter()
                        .filter(|v| !self.context.env.contains_key(*v))
                        .map(String::as_str)
                        .collect();
                    if !missing.is_empty() {
                        return Err(ExecError::RequiredEnvMissing {
                            vars: missing.join(", "),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn execute_op(&mut self, op: &Op) -> Result<(), ExecError> {
        match op {
            Op::RunProcess {
                command,
                args,
                env,
                stdout,
                stderr,
                ..
            } => {
                self.run_process(command, args, env, stdout, stderr)?;
            }

            Op::Echo { value, no_newline } => {
                let s = self.context.resolve_to_string(value);
                if *no_newline {
                    print!("{s}");
                } else {
                    println!("{s}");
                }
            }

            Op::BindVar { name, value, .. } => {
                let resolved = self.context.resolve(value);
                self.context.set_var(name, resolved);
            }

            Op::SetEnv { key, value } => {
                let resolved = self.context.resolve_to_string(value);
                self.context.set_env(key, &resolved);
            }

            Op::UnsetEnv { key } => {
                self.context.env.remove(key);
                // No std::env::remove_var — children receive ctx.env via cmd.envs()
            }

            Op::Cd { path } => {
                let new_path = if std::path::Path::new(path).is_absolute() {
                    std::path::PathBuf::from(path)
                } else {
                    self.context.cwd.join(path)
                };
                let canonical = new_path.canonicalize().map_err(|_| {
                    ExecError::CommandNotFound(format!("cd: {path}: no such directory"))
                })?;
                self.context.env.insert(
                    "OLDPWD".to_string(),
                    self.context.cwd.to_string_lossy().to_string(),
                );
                self.context.cwd = canonical;
            }

            // NOTE: Op::Binary not Op::BinOp
            Op::Bin {
                result_var,
                op,
                left,
                right,
            } => {
                let l = self.context.resolve(left);
                let r = self.context.resolve(right);
                let result = self.eval_binop(*op, &l, &r)?;
                self.context.set_var(result_var, result);
            }

            // NOTE: Op::Unary not Op::UnaryOp
            Op::Unary {
                result_var,
                op,
                operand,
            } => {
                let val = self.context.resolve(operand);
                let result = Self::eval_unary(*op, &val)?;
                self.context.set_var(result_var, result);
            }

            Op::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let cond = self.context.resolve(condition);
                let ops_to_run = if cond.is_truthy() { then_ops } else { else_ops };
                for op in ops_to_run.clone() {
                    self.execute_op(&op)?;
                }
            }

            Op::While {
                condition_ops,
                condition_var,
                body_ops,
            } => loop {
                for op in condition_ops {
                    self.execute_op(op)?;
                }
                let cond = self
                    .context
                    .vars
                    .get(condition_var)
                    .cloned()
                    .unwrap_or(Value::Bool(false));
                if !cond.is_truthy() {
                    break;
                }
                for op in body_ops {
                    self.execute_op(op)?;
                }
            },

            Op::Pipe { left, right } => {
                // v1: sequential — true concurrent pipe in later milestone
                // True piping requires spawning both concurrently with OS pipe
                tracing::warn!(
                    "Pipe is sequential in v1 — stdout of left is not connected to stdin of right"
                );
                self.execute_op(left)?;
                self.execute_op(right)?;
            }

            Op::RedirectOut { op, .. } => {
                // v1: execute without redirect — full implementation later
                tracing::warn!("Redirect not yet implemented");
                self.execute_op(op)?;
            }

            Op::RedirectIn { op, .. } => {
                tracing::warn!("Redirect not yet implemented");
                self.execute_op(op)?;
            }

            Op::Return { value } => {
                let val = self.context.resolve(value);
                if let Value::Int(code) = val {
                    #[allow(clippy::cast_possible_truncation)]
                    let exit_code = code as i32;
                    self.context.last_exit = exit_code;
                }
            }

            Op::Exit { code } => {
                std::process::exit(*code);
            }

            Op::LoadEnvFile { path } => {
                self.load_env_file(path)?;
            }

            Op::RequireEnv { vars } => {
                let missing: Vec<&str> = vars
                    .iter()
                    .filter(|v| !self.context.env.contains_key(*v))
                    .map(String::as_str)
                    .collect();
                if !missing.is_empty() {
                    return Err(ExecError::RequiredEnvMissing {
                        vars: missing.join(", "),
                    });
                }
            }

            Op::CallFn { name, args, .. } => {
                let resolved_args: Vec<String> = args
                    .iter()
                    .map(|a| self.context.resolve_to_string(a))
                    .collect();

                if let Some(cmd) = self.registry.get(name) {
                    let exit = cmd.run(&resolved_args, &mut self.context)?;
                    self.context.last_exit = exit;
                } else {
                    tracing::warn!("ForgeScript function '{}' not yet dispatchable", name);
                }
            }
        }

        // Strict mode: fail immediately on non-zero exit
        if self.context.strict_mode && self.context.last_exit != 0 {
            return Err(ExecError::CommandNotFound(format!(
                "strict mode: last command exited with code {}",
                self.context.last_exit
            )));
        }

        Ok(())
    }

    fn run_process(
        &mut self,
        command: &str,
        args: &[String],
        env_overrides: &[(String, String)],
        stdout: &StdioConfig,
        stderr: &StdioConfig,
    ) -> Result<(), ExecError> {
        // Built-ins run in-process
        if self.registry.is_builtin(command) {
            if let Some(cmd) = self.registry.get(command) {
                let exit = cmd.run(args, &mut self.context)?;
                self.context.last_exit = exit;
            }
            return Ok(());
        }

        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.current_dir(&self.context.cwd);
        cmd.envs(&self.context.env);
        for (k, v) in env_overrides {
            cmd.env(k, v);
        }
        cmd.stdin(Stdio::inherit());
        cmd.stdout(match stdout {
            StdioConfig::Inherit => Stdio::inherit(),
            StdioConfig::Piped => Stdio::piped(),
            StdioConfig::Null => Stdio::null(),
        });
        cmd.stderr(match stderr {
            StdioConfig::Inherit => Stdio::inherit(),
            StdioConfig::Piped => Stdio::piped(),
            StdioConfig::Null => Stdio::null(),
        });

        let status = cmd.status().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ExecError::CommandNotFound(command.to_string())
            } else {
                ExecError::Io(e)
            }
        })?;

        self.context.last_exit = status.code().unwrap_or(-1);
        Ok(())
    }

    fn load_env_file(&mut self, path: &str) -> Result<(), ExecError> {
        let content = std::fs::read_to_string(path).map_err(|_| ExecError::EnvFileNotFound {
            path: path.to_string(),
        })?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                // Do NOT overwrite existing vars — file provides defaults only
                if !self.context.env.contains_key(key) {
                    self.context.env.insert(key.to_string(), value.to_string());
                }
            }
        }
        Ok(())
    }

    fn eval_binop(&self, op: BinOpKind, left: &Value, right: &Value) -> Result<Value, ExecError> {
        match (op, left, right) {
            (BinOpKind::Add, Value::Int(a), Value::Int(b)) => match self.context.overflow_mode {
                OverflowMode::Panic => a
                    .checked_add(*b)
                    .map(Value::Int)
                    .ok_or(ExecError::IntegerOverflow),
                OverflowMode::Saturate => Ok(Value::Int(a.saturating_add(*b))),
                OverflowMode::Wrap => Ok(Value::Int(a.wrapping_add(*b))),
            },
            (BinOpKind::Sub, Value::Int(a), Value::Int(b)) => match self.context.overflow_mode {
                OverflowMode::Panic => a
                    .checked_sub(*b)
                    .map(Value::Int)
                    .ok_or(ExecError::IntegerOverflow),
                OverflowMode::Saturate => Ok(Value::Int(a.saturating_sub(*b))),
                OverflowMode::Wrap => Ok(Value::Int(a.wrapping_sub(*b))),
            },
            (BinOpKind::Mul, Value::Int(a), Value::Int(b)) => match self.context.overflow_mode {
                OverflowMode::Panic => a
                    .checked_mul(*b)
                    .map(Value::Int)
                    .ok_or(ExecError::IntegerOverflow),
                OverflowMode::Saturate => Ok(Value::Int(a.saturating_mul(*b))),
                OverflowMode::Wrap => Ok(Value::Int(a.wrapping_mul(*b))),
            },
            (BinOpKind::Div, Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(ExecError::DivisionByZero);
                }
                Ok(Value::Int(a / b))
            }
            (BinOpKind::Rem, Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(ExecError::DivisionByZero);
                }
                Ok(Value::Int(a % b))
            }
            (BinOpKind::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (BinOpKind::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            (BinOpKind::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            (BinOpKind::Div, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
            (BinOpKind::Eq, a, b) => Ok(Value::Bool(a == b)),
            (BinOpKind::Ne, a, b) => Ok(Value::Bool(a != b)),
            (BinOpKind::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a < b)),
            (BinOpKind::Le, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a <= b)),
            (BinOpKind::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a > b)),
            (BinOpKind::Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Bool(a >= b)),
            (BinOpKind::And, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a && *b)),
            (BinOpKind::Or, Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(*a || *b)),
            (BinOpKind::Concat, Value::Str(a), Value::Str(b)) => Ok(Value::Str(format!("{a}{b}"))),
            _ => Err(ExecError::TypeError {
                op: format!("{op:?}"),
                left: format!("{left:?}"),
                right: format!("{right:?}"),
            }),
        }
    }

    fn eval_unary(op: UnaryOpKind, val: &Value) -> Result<Value, ExecError> {
        match (op, val) {
            (UnaryOpKind::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
            (UnaryOpKind::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
            (UnaryOpKind::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
            _ => Err(ExecError::TypeError {
                op: format!("{op:?}"),
                left: format!("{val:?}"),
                right: String::new(),
            }),
        }
    }
}

fn current_platform() -> Platform {
    if cfg!(target_os = "linux") {
        Platform::Linux
    } else if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Unix
    }
}

fn version_satisfies(current: &str, required: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let p: Vec<u32> = s.split('.').filter_map(|x| x.parse().ok()).collect();
        (
            p.first().copied().unwrap_or(0),
            p.get(1).copied().unwrap_or(0),
            p.get(2).copied().unwrap_or(0),
        )
    };
    parse(current) >= parse(required)
}

fn parse_duration(s: &str) -> std::time::Duration {
    if let Some(n) = s.strip_suffix('s').and_then(|n| n.parse::<u64>().ok()) {
        return std::time::Duration::from_secs(n);
    }
    if let Some(n) = s.strip_suffix('m').and_then(|n| n.parse::<u64>().ok()) {
        return std::time::Duration::from_secs(n * 60);
    }
    if let Some(n) = s.strip_suffix('h').and_then(|n| n.parse::<u64>().ok()) {
        return std::time::Duration::from_secs(n * 3600);
    }
    std::time::Duration::from_secs(u64::MAX)
}

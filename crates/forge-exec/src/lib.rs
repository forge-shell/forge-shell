// crates/forge-exec/src/mod
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod builtins;
pub mod context;
pub mod error;
pub mod executor;

pub use context::ShellContext;
pub use error::ExecError;
pub use executor::Executor;

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ast::{Directive, DirectiveKind, JobLimit, OverflowMode, Platform};
    use forge_backend::plan::{ExecutionPlan, Op, Value};
    use forge_types::Span;

    fn directive(kind: DirectiveKind) -> Directive {
        Directive {
            kind,
            span: Span::default(),
        }
    }

    fn run(ops: Vec<Op>) -> Result<i32, ExecError> {
        Executor::new(ShellContext::new()).run(&ExecutionPlan::new(ops))
    }

    #[test]
    fn test_empty_plan_exits_zero() {
        assert_eq!(run(vec![]).unwrap(), 0);
    }

    #[test]
    fn test_bind_var() {
        let mut exec = Executor::new(ShellContext::new());
        exec.run(&ExecutionPlan::new(vec![Op::BindVar {
            name: "x".to_string(),
            mutable: false,
            value: Value::Int(42),
        }]))
        .unwrap();
        assert_eq!(exec.context.vars.get("x"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_set_env() {
        let mut exec = Executor::new(ShellContext::new());
        exec.run(&ExecutionPlan::new(vec![Op::SetEnv {
            key: "MY_VAR".to_string(),
            value: Value::Str("hello".to_string()),
        }]))
        .unwrap();
        assert_eq!(
            exec.context.env.get("MY_VAR").map(String::as_str),
            Some("hello")
        );
    }

    #[test]
    fn test_if_truthy_branch() {
        let mut exec = Executor::new(ShellContext::new());
        exec.run(&ExecutionPlan::new(vec![Op::If {
            condition: Value::Bool(true),
            then_ops: vec![Op::BindVar {
                name: "result".to_string(),
                mutable: false,
                value: Value::Int(1),
            }],
            else_ops: vec![Op::BindVar {
                name: "result".to_string(),
                mutable: false,
                value: Value::Int(2),
            }],
        }]))
        .unwrap();
        assert_eq!(exec.context.vars.get("result"), Some(&Value::Int(1)));
    }

    #[test]
    fn test_if_falsy_branch() {
        let mut exec = Executor::new(ShellContext::new());
        exec.run(&ExecutionPlan::new(vec![Op::If {
            condition: Value::Bool(false),
            then_ops: vec![Op::BindVar {
                name: "result".to_string(),
                mutable: false,
                value: Value::Int(1),
            }],
            else_ops: vec![Op::BindVar {
                name: "result".to_string(),
                mutable: false,
                value: Value::Int(2),
            }],
        }]))
        .unwrap();
        assert_eq!(exec.context.vars.get("result"), Some(&Value::Int(2)));
    }

    #[test]
    fn test_require_env_present() {
        let mut ctx = ShellContext::new();
        ctx.env
            .insert("REQUIRED_VAR".to_string(), "value".to_string());
        let result = Executor::new(ctx).run(&ExecutionPlan::new(vec![Op::RequireEnv {
            vars: vec!["REQUIRED_VAR".to_string()],
        }]));
        assert!(result.is_ok());
    }

    #[test]
    fn test_require_env_missing() {
        let mut ctx = ShellContext::new();
        ctx.env.remove("DEFINITELY_NOT_SET");
        let result = Executor::new(ctx).run(&ExecutionPlan::new(vec![Op::RequireEnv {
            vars: vec!["DEFINITELY_NOT_SET".to_string()],
        }]));
        assert!(matches!(result, Err(ExecError::RequiredEnvMissing { .. })));
    }

    #[test]
    fn test_var_ref_resolved() {
        let mut ctx = ShellContext::new();
        ctx.set_var("greeting", Value::Str("hello".to_string()));
        let resolved = ctx.resolve(&Value::VarRef("greeting".to_string()));
        assert_eq!(resolved, Value::Str("hello".to_string()));
    }

    #[test]
    fn test_env_ref_resolved() {
        let mut ctx = ShellContext::new();
        ctx.env.insert("MY_KEY".to_string(), "my_val".to_string());
        let resolved = ctx.resolve(&Value::EnvRef("MY_KEY".to_string()));
        assert_eq!(resolved, Value::Str("my_val".to_string()));
    }

    #[test]
    fn test_unset_env() {
        let mut exec = Executor::new(ShellContext::new());
        exec.context
            .env
            .insert("TO_REMOVE".to_string(), "val".to_string());
        exec.run(&ExecutionPlan::new(vec![Op::UnsetEnv {
            key: "TO_REMOVE".to_string(),
        }]))
        .unwrap();
        assert!(!exec.context.env.contains_key("TO_REMOVE"));
    }

    #[test]
    fn test_shell_context_is_not_send() {
        // Compile-time check — this function must NOT compile if uncommented:
        // fn assert_send<T: Send>() {}
        // fn check() { assert_send::<ShellContext>(); }
        //
        // PhantomData<*const ()> makes ShellContext !Send.
        // This test documents the invariant.
    }

    // ── enforce_directives tests ─────────────────────────────────────────────

    #[test]
    fn test_enforce_min_version_satisfied() {
        let mut exec = Executor::new(ShellContext::new());
        let result =
            exec.enforce_directives(&[directive(DirectiveKind::MinVersion("0.0.1".to_string()))]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_min_version_not_met() {
        let mut exec = Executor::new(ShellContext::new());
        let result =
            exec.enforce_directives(&[directive(DirectiveKind::MinVersion("999.0.0".to_string()))]);
        assert!(matches!(result, Err(ExecError::MinVersionNotMet { .. })));
    }

    #[test]
    fn test_enforce_platform_all_passes() {
        let mut exec = Executor::new(ShellContext::new());
        let result =
            exec.enforce_directives(&[directive(DirectiveKind::Platform(vec![Platform::All]))]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_platform_current_supported() {
        let current = if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Windows
        };
        let mut exec = Executor::new(ShellContext::new());
        let result = exec.enforce_directives(&[directive(DirectiveKind::Platform(vec![current]))]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_platform_not_supported() {
        let impossible = if cfg!(target_os = "linux") {
            Platform::Windows
        } else {
            Platform::Linux // Windows and macOS cannot both run Linux-only scripts
        };
        let mut exec = Executor::new(ShellContext::new());
        let result =
            exec.enforce_directives(&[directive(DirectiveKind::Platform(vec![impossible]))]);
        assert!(matches!(
            result,
            Err(ExecError::PlatformNotSupported { .. })
        ));
    }

    #[test]
    fn test_enforce_require_env_present() {
        let mut ctx = ShellContext::new();
        ctx.env.insert("REQUIRED".to_string(), "yes".to_string());
        let mut exec = Executor::new(ctx);
        let result = exec.enforce_directives(&[directive(DirectiveKind::RequireEnv(vec![
            "REQUIRED".to_string(),
        ]))]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_require_env_missing() {
        let mut exec = Executor::new(ShellContext::new());
        let result = exec.enforce_directives(&[directive(DirectiveKind::RequireEnv(vec![
            "DEFINITELY_ABSENT_VAR".to_string(),
        ]))]);
        assert!(matches!(result, Err(ExecError::RequiredEnvMissing { .. })));
    }

    #[test]
    fn test_enforce_overflow_mode_applied() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Overflow(OverflowMode::Saturate))])
            .unwrap();
        assert_eq!(exec.context.overflow_mode, OverflowMode::Saturate);
    }

    #[test]
    fn test_enforce_strict_enabled() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Strict(true))])
            .unwrap();
        assert!(exec.context.strict_mode);
    }

    #[test]
    fn test_enforce_strict_disabled() {
        let mut ctx = ShellContext::new();
        ctx.strict_mode = true;
        let mut exec = Executor::new(ctx);
        exec.enforce_directives(&[directive(DirectiveKind::Strict(false))])
            .unwrap();
        assert!(!exec.context.strict_mode);
    }

    #[test]
    fn test_enforce_timeout_seconds() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Timeout("30s".to_string()))])
            .unwrap();
        assert_eq!(
            exec.context.timeout,
            Some(std::time::Duration::from_secs(30))
        );
    }

    #[test]
    fn test_enforce_timeout_minutes() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Timeout("2m".to_string()))])
            .unwrap();
        assert_eq!(
            exec.context.timeout,
            Some(std::time::Duration::from_secs(120))
        );
    }

    #[test]
    fn test_enforce_jobs_auto() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Jobs(JobLimit::Auto))])
            .unwrap();
        assert!(exec.context.max_jobs >= 1);
    }

    #[test]
    fn test_enforce_jobs_count() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::Jobs(JobLimit::Count(4)))])
            .unwrap();
        assert_eq!(exec.context.max_jobs, 4);
    }

    #[test]
    fn test_enforce_env_file_loads_vars() {
        use std::io::Write as _;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "LOADED_KEY=loaded_value").unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[directive(DirectiveKind::EnvFile(path))])
            .unwrap();
        assert_eq!(
            exec.context.env.get("LOADED_KEY").map(String::as_str),
            Some("loaded_value")
        );
    }

    #[test]
    fn test_enforce_env_file_not_found() {
        let mut exec = Executor::new(ShellContext::new());
        let result = exec.enforce_directives(&[directive(DirectiveKind::EnvFile(
            "/tmp/forge_test_nonexistent_99999.env".to_string(),
        ))]);
        assert!(matches!(result, Err(ExecError::EnvFileNotFound { .. })));
    }

    #[test]
    fn test_enforce_metadata_directives_ignored() {
        let mut exec = Executor::new(ShellContext::new());
        let result = exec.enforce_directives(&[
            directive(DirectiveKind::UnixShebang("/usr/bin/env forge".to_string())),
            directive(DirectiveKind::Description("a script".to_string())),
            directive(DirectiveKind::Author("someone".to_string())),
            directive(DirectiveKind::Override("ls".to_string())),
            directive(DirectiveKind::Unknown {
                key: "future-key".to_string(),
                value: "ignored".to_string(),
            }),
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_enforce_multiple_directives_combined() {
        let mut exec = Executor::new(ShellContext::new());
        exec.enforce_directives(&[
            directive(DirectiveKind::MinVersion("0.0.1".to_string())),
            directive(DirectiveKind::Overflow(OverflowMode::Wrap)),
            directive(DirectiveKind::Strict(true)),
            directive(DirectiveKind::Jobs(JobLimit::Count(2))),
            directive(DirectiveKind::Timeout("1s".to_string())),
        ])
        .unwrap();
        assert_eq!(exec.context.overflow_mode, OverflowMode::Wrap);
        assert!(exec.context.strict_mode);
        assert_eq!(exec.context.max_jobs, 2);
        assert_eq!(
            exec.context.timeout,
            Some(std::time::Duration::from_secs(1))
        );
    }
}

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
    use forge_backend::plan::{ExecutionPlan, Op, Value};

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
            exec.context.env.get("MY_VAR").map(|s| s.as_str()),
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
}

use crate::builtins::BuiltinCommand;
use crate::{ExecError, ShellContext};

pub struct Unset;

impl BuiltinCommand for Unset {
    fn name(&self) -> &'static str {
        "unset"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        if args.is_empty() {
            return Err(ExecError::InvalidArgument(
                "unset: missing variable name".into(),
            ));
        }
        for name in args {
            ctx.remove_env(name);
        }
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;

    fn ctx() -> ShellContext {
        ShellContext::new()
    }

    #[test]
    fn test_unset_removes_var() {
        let mut ctx = ctx();
        ctx.env.insert("MY_VAR".into(), "val".into());
        assert_eq!(Unset.run(&["MY_VAR".into()], &mut ctx).unwrap(), 0);
        assert!(!ctx.env.contains_key("MY_VAR"));
    }

    #[test]
    fn test_unset_no_args() {
        assert!(matches!(
            Unset.run(&[], &mut ctx()),
            Err(ExecError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_unset_nonexistent_is_ok() {
        assert_eq!(Unset.run(&["__never_set__".into()], &mut ctx()).unwrap(), 0);
    }
}

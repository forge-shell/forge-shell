use crate::builtins::BuiltinCommand;
use crate::{ExecError, ShellContext};

pub struct Set;

impl BuiltinCommand for Set {
    fn name(&self) -> &'static str {
        "set"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        match args {
            // set KEY=VALUE
            [kv] if kv.contains('=') => {
                let (k, v) = kv.split_once('=').unwrap();
                ctx.set_env(k, v);
                Ok(0)
            }
            // set KEY VALUE
            [key, value] => {
                ctx.set_env(key, value);
                Ok(0)
            }
            _ => Err(ExecError::InvalidArgument(
                "set: usage: set NAME VALUE  or  set NAME=VALUE".into(),
            )),
        }
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
    fn test_set_two_args() {
        let mut ctx = ctx();
        assert_eq!(Set.run(&["FOO".into(), "bar".into()], &mut ctx).unwrap(), 0);
        assert_eq!(ctx.env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn test_set_kv_form() {
        let mut ctx = ctx();
        assert_eq!(Set.run(&["FOO=bar".into()], &mut ctx).unwrap(), 0);
        assert_eq!(ctx.env.get("FOO").map(String::as_str), Some("bar"));
    }

    #[test]
    fn test_set_bad_args() {
        assert!(matches!(
            Set.run(&[], &mut ctx()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

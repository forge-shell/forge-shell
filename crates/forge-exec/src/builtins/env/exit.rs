use crate::builtins::BuiltinCommand;
use crate::{ExecError, ShellContext};

pub struct Exit;

impl BuiltinCommand for Exit {
    fn name(&self) -> &'static str {
        "exit"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let code = match args.first() {
            None => ctx.last_exit,
            Some(s) => s.parse::<i32>().map_err(|_| {
                ExecError::InvalidArgument(format!("exit: {s}: numeric argument required"))
            })?,
        };
        std::process::exit(code);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;

    #[test]
    fn test_exit_invalid_arg() {
        let mut ctx = ShellContext::new();
        assert!(matches!(
            Exit.run(&["abc".into()], &mut ctx),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

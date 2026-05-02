use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};

pub struct Which;

impl BuiltinCommand for Which {
    fn name(&self) -> &'static str {
        "which"
    }

    fn run(&self, args: &[String], _ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let all = utils::has_flag(args, "-a");
        let silent = utils::has_flag(args, "-s");
        let names = utils::positional_args(args, &[]);

        if names.is_empty() {
            return Err(ExecError::InvalidArgument("which: missing argument".into()));
        }

        let mut exit_code = 0i32;
        for name in names {
            if all {
                if let Ok(paths) = which::which_all(name) {
                    let mut found = false;
                    for p in paths {
                        found = true;
                        if !silent {
                            println!("{}", p.display());
                        }
                    }
                    if !found {
                        if !silent {
                            eprintln!("which: {name}: not found");
                        }
                        exit_code = 1;
                    }
                } else {
                    if !silent {
                        eprintln!("which: {name}: not found");
                    }
                    exit_code = 1;
                }
            } else if let Ok(p) = which::which(name) {
                if !silent {
                    println!("{}", p.display());
                }
            } else {
                if !silent {
                    eprintln!("which: {name}: not found");
                }
                exit_code = 1;
            }
        }
        Ok(exit_code)
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
    fn test_which_no_args() {
        assert!(Which.run(&[], &mut ctx()).is_err());
    }

    #[test]
    fn test_which_missing_command() {
        assert_eq!(
            Which
                .run(&["__no_such_cmd_xyz__".into()], &mut ctx())
                .unwrap(),
            1
        );
    }
}

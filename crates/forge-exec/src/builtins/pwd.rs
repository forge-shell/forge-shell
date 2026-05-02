use crate::builtins::BuiltinCommand;
use crate::{ExecError, ShellContext};

pub struct Pwd;

impl BuiltinCommand for Pwd {
    fn name(&self) -> &'static str {
        "pwd"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let want_physical = args.iter().any(|a| a == "-P" || a == "--physical")
            || (!args.iter().any(|a| a == "-L" || a == "--logical") && default_physical());

        let path = if want_physical {
            ctx.cwd.canonicalize()?
        } else {
            // Logical: prefer $PWD env var (may contain unresolved symlinks)
            ctx.env
                .get("PWD")
                .map_or_else(|| ctx.cwd.clone(), std::path::PathBuf::from)
        };

        println!("{}", path.display());
        Ok(0)
    }
}

/// Returns true when the platform default is physical resolution.
/// macOS defaults to logical (-L); Linux and Windows default to physical (-P).
const fn default_physical() -> bool {
    !cfg!(target_os = "macos")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> ShellContext {
        ShellContext::new()
    }

    #[test]
    fn test_pwd_exits_zero() {
        assert_eq!(Pwd.run(&[], &mut ctx()).unwrap(), 0);
    }

    #[test]
    fn test_pwd_logical() {
        let mut ctx = ctx();
        ctx.env.insert("PWD".into(), "/fake/logical/path".into());
        // -L should not error even with a fake path
        assert_eq!(Pwd.run(&["-L".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_pwd_physical() {
        // -P canonicalizes; current dir must exist
        assert_eq!(Pwd.run(&["-P".into()], &mut ctx()).unwrap(), 0);
    }
}

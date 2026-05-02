use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};

pub struct Rmdir;

impl BuiltinCommand for Rmdir {
    fn name(&self) -> &'static str {
        "rmdir"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let parents = utils::has_flag(args, "-p");
        let verbose = utils::has_flag(args, "-v");
        let dirs = utils::positional_args(args, &[]);

        if dirs.is_empty() {
            return Err(ExecError::InvalidArgument("rmdir: missing operand".into()));
        }

        for dir in dirs {
            let path = resolve(dir, &ctx.cwd);
            if parents {
                // Remove the full path component chain, stopping at non-empty
                let mut current = path.clone();
                loop {
                    std::fs::remove_dir(&current)?;
                    if verbose {
                        println!("rmdir: removing directory, '{}'", current.display());
                    }
                    match current.parent() {
                        Some(parent) if parent != std::path::Path::new("") => {
                            current = parent.to_path_buf();
                        }
                        _ => break,
                    }
                }
            } else {
                std::fs::remove_dir(&path)?;
                if verbose {
                    println!("rmdir: removing directory, '{}'", path.display());
                }
            }
        }
        Ok(0)
    }
}

fn resolve(path: &str, cwd: &std::path::Path) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;
    use tempfile::TempDir;

    fn ctx_in(tmp: &TempDir) -> ShellContext {
        let mut ctx = ShellContext::new();
        ctx.cwd = tmp.path().to_path_buf();
        ctx
    }

    #[test]
    fn test_rmdir_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("empty");
        std::fs::create_dir(&sub).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Rmdir.run(&["empty".into()], &mut ctx).unwrap(), 0);
        assert!(!sub.exists());
    }

    #[test]
    fn test_rmdir_nonempty_errors() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("file.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert!(
            Rmdir
                .run(&[tmp.path().to_str().unwrap().into()], &mut ctx)
                .is_err()
        );
    }

    #[test]
    fn test_rmdir_no_args_errors() {
        assert!(matches!(
            Rmdir.run(&[], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

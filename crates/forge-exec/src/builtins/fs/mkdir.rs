use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::Path;

pub struct Mkdir;

impl BuiltinCommand for Mkdir {
    fn name(&self) -> &'static str {
        "mkdir"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let parents = utils::has_flag(args, "-p");
        let verbose = utils::has_flag(args, "-v");
        let mode_str = utils::flag_value(args, "-m");
        let dirs = utils::positional_args(args, &["-m"]);

        if dirs.is_empty() {
            return Err(ExecError::InvalidArgument("mkdir: missing operand".into()));
        }

        for dir in dirs {
            let path = resolve(dir, &ctx.cwd);
            if parents {
                std::fs::create_dir_all(&path)?;
            } else {
                std::fs::create_dir(&path)?;
            }
            if let Some(mode) = mode_str {
                apply_mode(&path, mode);
            }
            if verbose {
                println!("mkdir: created directory '{}'", path.display());
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

#[cfg(unix)]
fn apply_mode(path: &Path, mode_str: &str) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(m) = u32::from_str_radix(mode_str.trim_start_matches('0'), 8) {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(m));
    }
}

#[cfg(not(unix))]
fn apply_mode(_path: &Path, _mode_str: &str) {}

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
    fn test_mkdir_creates_dir() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Mkdir.run(&["newdir".into()], &mut ctx).unwrap(), 0);
        assert!(tmp.path().join("newdir").is_dir());
    }

    #[test]
    fn test_mkdir_parents() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Mkdir.run(&["-p".into(), "a/b/c".into()], &mut ctx).unwrap(),
            0
        );
        assert!(tmp.path().join("a/b/c").is_dir());
    }

    #[test]
    fn test_mkdir_no_args_errors() {
        let mut ctx = ShellContext::new();
        assert!(matches!(
            Mkdir.run(&[], &mut ctx),
            Err(ExecError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_mkdir_exists_no_parents_errors() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert!(
            Mkdir
                .run(&[tmp.path().to_str().unwrap().into()], &mut ctx)
                .is_err()
        );
    }
}

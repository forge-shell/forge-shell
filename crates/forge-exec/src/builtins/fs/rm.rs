use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::ErrorKind;
use std::path::PathBuf;

pub struct Rm;

impl BuiltinCommand for Rm {
    fn name(&self) -> &'static str {
        "rm"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let force = utils::has_flag(args, "-f");
        let recursive = utils::has_flag(args, "-r") || utils::has_flag(args, "-R");
        let verbose = utils::has_flag(args, "-v");
        let dir_mode = utils::has_flag(args, "-d"); // remove empty dirs like rmdir
        let targets = utils::positional_args(args, &[]);

        if targets.is_empty() && !force {
            return Err(ExecError::InvalidArgument("rm: missing operand".into()));
        }

        let mut exit_code = 0i32;
        for t in targets {
            let path = resolve(t, &ctx.cwd);
            // Guard against removing / . or ..
            if is_unsafe(&path) {
                eprintln!("rm: refusing to remove '{}'", path.display());
                exit_code = 1;
                continue;
            }
            match remove_path(&path, recursive, dir_mode, verbose, force) {
                Ok(()) => {}
                Err(e) if force && e.kind() == ErrorKind::NotFound => {}
                Err(e) => {
                    eprintln!("rm: cannot remove '{}': {e}", path.display());
                    exit_code = 1;
                }
            }
        }
        Ok(exit_code)
    }
}

#[allow(clippy::fn_params_excessive_bools)]
fn remove_path(
    path: &std::path::Path,
    recursive: bool,
    dir_mode: bool,
    verbose: bool,
    _force: bool,
) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        if recursive {
            std::fs::remove_dir_all(path)?;
        } else if dir_mode {
            std::fs::remove_dir(path)?;
        } else {
            return Err(std::io::Error::new(
                ErrorKind::IsADirectory,
                "is a directory",
            ));
        }
    } else {
        std::fs::remove_file(path)?;
    }
    if verbose {
        println!("removed '{}'", path.display());
    }
    Ok(())
}

fn is_unsafe(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s == "/" || s == "." || s == ".."
}

fn resolve(path: &str, cwd: &std::path::Path) -> PathBuf {
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
    fn test_rm_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("a.txt");
        std::fs::write(&f, b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Rm.run(&["a.txt".into()], &mut ctx).unwrap(), 0);
        assert!(!f.exists());
    }

    #[test]
    fn test_rm_recursive() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("f.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Rm.run(&["-r".into(), "sub".into()], &mut ctx).unwrap(), 0);
        assert!(!sub.exists());
    }

    #[test]
    fn test_rm_force_missing_ok() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Rm.run(&["-f".into(), "nonexistent".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_rm_dir_without_r_fails() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Rm.run(&["sub".into()], &mut ctx).unwrap(), 1);
    }

    #[test]
    fn test_rm_no_args_errors() {
        assert!(matches!(
            Rm.run(&[], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

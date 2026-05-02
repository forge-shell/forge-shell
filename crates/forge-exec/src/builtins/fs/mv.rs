use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Mv;

impl BuiltinCommand for Mv {
    fn name(&self) -> &'static str {
        "mv"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let no_overwrite = utils::has_flag(args, "-n");
        let verbose = utils::has_flag(args, "-v");
        let positional = utils::positional_args(args, &[]);

        if positional.len() < 2 {
            return Err(ExecError::InvalidArgument(
                "mv: missing destination operand".into(),
            ));
        }

        let dest_str = *positional.last().unwrap();
        let sources = &positional[..positional.len() - 1];
        let dest = resolve(dest_str, &ctx.cwd);

        for src_str in sources {
            let src = resolve(src_str, &ctx.cwd);
            let final_dest = if dest.is_dir() {
                dest.join(src.file_name().unwrap_or_default())
            } else {
                dest.clone()
            };

            if no_overwrite && final_dest.exists() {
                continue;
            }

            if let Err(e) = std::fs::rename(&src, &final_dest) {
                // Cross-device move: copy then delete
                if e.raw_os_error() == Some(cross_device_error()) {
                    copy_then_remove(&src, &final_dest)?;
                } else {
                    return Err(e.into());
                }
            }
            if verbose {
                println!("'{}' -> '{}'", src.display(), final_dest.display());
            }
        }
        Ok(0)
    }
}

fn copy_then_remove(src: &Path, dst: &Path) -> Result<(), ExecError> {
    let meta = std::fs::symlink_metadata(src)?;
    if meta.is_dir() {
        for entry in walkdir::WalkDir::new(src) {
            let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;
            let rel = entry.path().strip_prefix(src).unwrap();
            let dp = dst.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dp)?;
            } else {
                std::fs::copy(entry.path(), &dp)?;
            }
        }
        std::fs::remove_dir_all(src)?;
    } else {
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }
    Ok(())
}

#[cfg(unix)]
fn cross_device_error() -> i32 {
    18
} // EXDEV

#[cfg(windows)]
fn cross_device_error() -> i32 {
    17
} // ERROR_NOT_SAME_DEVICE

#[cfg(not(any(unix, windows)))]
fn cross_device_error() -> i32 {
    -1
}

fn resolve(path: &str, cwd: &Path) -> PathBuf {
    let p = Path::new(path);
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
    fn test_mv_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Mv.run(&["a.txt".into(), "b.txt".into()], &mut ctx).unwrap(),
            0
        );
        assert!(!tmp.path().join("a.txt").exists());
        assert!(tmp.path().join("b.txt").exists());
    }

    #[test]
    fn test_mv_into_dir() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("dst");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Mv.run(&["a.txt".into(), "dst".into()], &mut ctx).unwrap(),
            0
        );
        assert!(sub.join("a.txt").exists());
    }

    #[test]
    fn test_mv_no_overwrite() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"original").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"existing").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Mv.run(&["-n".into(), "a.txt".into(), "b.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
        assert_eq!(
            std::fs::read(tmp.path().join("b.txt")).unwrap(),
            b"existing"
        );
    }

    #[test]
    fn test_mv_missing_dest_errors() {
        assert!(matches!(
            Mv.run(&["a".into()], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Cp;

impl BuiltinCommand for Cp {
    fn name(&self) -> &'static str {
        "cp"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let recursive = utils::has_flag(args, "-R")
            || utils::has_flag(args, "-r")
            || utils::has_flag(args, "-a");
        let no_overwrite = utils::has_flag(args, "-n");
        let verbose = utils::has_flag(args, "-v");
        let preserve = utils::has_flag(args, "-p") || utils::has_flag(args, "-a");
        let positional = utils::positional_args(args, &[]);

        if positional.len() < 2 {
            return Err(ExecError::InvalidArgument(
                "cp: missing destination operand".into(),
            ));
        }

        let dest_str = *positional.last().unwrap();
        let sources = &positional[..positional.len() - 1];
        let dest = resolve(dest_str, &ctx.cwd);

        for src_str in sources {
            let src = resolve(src_str, &ctx.cwd);
            copy_item(&src, &dest, recursive, no_overwrite, verbose, preserve)?;
        }
        Ok(0)
    }
}

#[allow(clippy::fn_params_excessive_bools)]
fn copy_item(
    src: &Path,
    dest: &Path,
    recursive: bool,
    no_overwrite: bool,
    verbose: bool,
    preserve: bool,
) -> Result<(), ExecError> {
    let meta = std::fs::symlink_metadata(src)?;

    if meta.is_dir() {
        if !recursive {
            return Err(ExecError::InvalidArgument(format!(
                "cp: -r not specified; omitting directory '{}'",
                src.display()
            )));
        }
        let target = if dest.is_dir() {
            dest.join(src.file_name().unwrap_or_default())
        } else {
            dest.to_path_buf()
        };
        for entry in WalkDir::new(src) {
            let entry = entry.map_err(|e| std::io::Error::other(e.to_string()))?;
            let rel = entry.path().strip_prefix(src).unwrap();
            let dst_path = target.join(rel);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(&dst_path)?;
            } else {
                if no_overwrite && dst_path.exists() {
                    continue;
                }
                std::fs::copy(entry.path(), &dst_path)?;
                if verbose {
                    println!("'{}' -> '{}'", entry.path().display(), dst_path.display());
                }
                if preserve {
                    preserve_times(entry.path(), &dst_path);
                }
            }
        }
    } else {
        let final_dest = if dest.is_dir() {
            dest.join(src.file_name().unwrap_or_default())
        } else {
            dest.to_path_buf()
        };
        if no_overwrite && final_dest.exists() {
            return Ok(());
        }
        std::fs::copy(src, &final_dest)?;
        if verbose {
            println!("'{}' -> '{}'", src.display(), final_dest.display());
        }
        if preserve {
            preserve_times(src, &final_dest);
        }
    }
    Ok(())
}

fn preserve_times(src: &Path, dst: &Path) {
    if let (Ok(smeta), Ok(_)) = (src.metadata(), dst.metadata()) {
        if let (Ok(atime), Ok(mtime)) = (smeta.accessed(), smeta.modified()) {
            let _ = filetime::set_file_times(
                dst,
                filetime::FileTime::from_system_time(atime),
                filetime::FileTime::from_system_time(mtime),
            );
        }
    }
}

fn resolve(path: &str, cwd: &std::path::Path) -> PathBuf {
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
    fn test_cp_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Cp.run(&["a.txt".into(), "b.txt".into()], &mut ctx).unwrap(),
            0
        );
        assert_eq!(std::fs::read(tmp.path().join("b.txt")).unwrap(), b"hello");
    }

    #[test]
    fn test_cp_recursive() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("f.txt"), b"data").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Cp.run(&["-r".into(), "src".into(), "dst".into()], &mut ctx)
                .unwrap(),
            0
        );
        assert!(tmp.path().join("dst/f.txt").exists());
    }

    #[test]
    fn test_cp_dir_without_r_fails() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert!(Cp.run(&["sub".into(), "dst".into()], &mut ctx).is_err());
    }

    #[test]
    fn test_cp_missing_dest_errors() {
        assert!(matches!(
            Cp.run(&["a".into()], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

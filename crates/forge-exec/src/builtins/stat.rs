use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Stat;

impl BuiltinCommand for Stat {
    fn name(&self) -> &'static str {
        "stat"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let follow_links = utils::has_flag(args, "-L") || utils::has_flag(args, "--dereference");
        let terse = utils::has_flag(args, "-t") || utils::has_flag(args, "--terse");
        let shell_fmt = utils::has_flag(args, "-s");
        let targets = utils::positional_args(args, &["-f", "--format", "--printf", "-t"]);

        if targets.is_empty() {
            return Err(ExecError::InvalidArgument("stat: missing operand".into()));
        }

        for t in targets {
            let path = resolve(t, &ctx.cwd);
            let meta = if follow_links {
                std::fs::metadata(&path)?
            } else {
                std::fs::symlink_metadata(&path)?
            };

            if terse {
                print_terse(&path, &meta);
            } else if shell_fmt {
                print_shell(&path, &meta);
            } else {
                print_verbose(&path, &meta)?;
            }
        }
        Ok(0)
    }
}

#[allow(clippy::unnecessary_wraps)]
fn print_verbose(path: &Path, meta: &std::fs::Metadata) -> Result<(), ExecError> {
    let is_dir = meta.is_dir();
    let is_link = meta.file_type().is_symlink();
    let size = meta.len();
    let mode = get_mode(meta);
    let perm = utils::format_mode(mode, is_dir, is_link);
    let atime = meta.accessed().map(utils::format_time).unwrap_or_default();
    let mtime = meta.modified().map(utils::format_time).unwrap_or_default();
    let ctime = get_ctime(meta);
    let inode = get_inode(meta);
    let nlinks = get_nlinks(meta);
    let uid = get_uid(meta);
    let gid = get_gid(meta);
    let blocks = get_blocks(meta);
    let blksize = get_blksize(meta);
    let ftype = if is_link {
        "symbolic link"
    } else if is_dir {
        "directory"
    } else {
        "regular file"
    };

    println!("  File: {}", path.display());
    println!("  Size: {size:<14} Blocks: {blocks:<10} IO Block: {blksize:<6} {ftype}");
    println!("Device: {inode:<14} Inode: {inode:<11} Links: {nlinks}");
    println!("Access: ({mode:04o}/{perm})  Uid: ({uid:5})  Gid: ({gid:5})");
    println!("Access: {atime}");
    println!("Modify: {mtime}");
    println!("Change: {ctime}");
    println!(" Birth: -");
    Ok(())
}

fn print_terse(path: &Path, meta: &std::fs::Metadata) {
    let size = meta.len();
    let mode = get_mode(meta);
    let inode = get_inode(meta);
    let nlinks = get_nlinks(meta);
    let uid = get_uid(meta);
    let gid = get_gid(meta);
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    println!(
        "{} {} {} {} {} {} {} {}",
        path.display(),
        size,
        mode,
        inode,
        nlinks,
        uid,
        gid,
        mtime
    );
}

fn print_shell(path: &Path, meta: &std::fs::Metadata) {
    println!("st_size={}", meta.len());
    println!("st_ino={}", get_inode(meta));
    println!("st_mode={}", get_mode(meta));
    println!("st_nlink={}", get_nlinks(meta));
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    println!("st_mtime={mtime}");
    println!("# {}", path.display());
}

#[cfg(unix)]
fn get_mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}
#[cfg(not(unix))]
fn get_mode(meta: &std::fs::Metadata) -> u32 {
    if meta.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

#[cfg(unix)]
fn get_inode(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino()
}
#[cfg(not(unix))]
fn get_inode(_meta: &std::fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn get_nlinks(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.nlink()
}
#[cfg(not(unix))]
fn get_nlinks(_meta: &std::fs::Metadata) -> u64 {
    1
}

#[cfg(unix)]
fn get_uid(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.uid()
}
#[cfg(not(unix))]
fn get_uid(_meta: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn get_gid(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.gid()
}
#[cfg(not(unix))]
fn get_gid(_meta: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn get_blocks(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blocks()
}
#[cfg(not(unix))]
fn get_blocks(meta: &std::fs::Metadata) -> u64 {
    (meta.len() + 511) / 512
}

#[cfg(unix)]
fn get_blksize(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.blksize()
}
#[cfg(not(unix))]
fn get_blksize(_meta: &std::fs::Metadata) -> u64 {
    4096
}

fn get_ctime(meta: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let secs = meta.ctime();
        utils::format_time(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs.unsigned_abs()),
        )
    }
    #[cfg(not(unix))]
    {
        meta.modified().map(utils::format_time).unwrap_or_default()
    }
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
    fn test_stat_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Stat.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_stat_no_args_errors() {
        assert!(matches!(
            Stat.run(&[], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }

    #[test]
    fn test_stat_missing_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/tmp");
        assert!(Stat.run(&["__no_such_file__".into()], &mut ctx).is_err());
    }
}

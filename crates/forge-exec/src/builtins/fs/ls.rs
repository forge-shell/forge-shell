use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Ls;

impl BuiltinCommand for Ls {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let show_all = utils::has_flag(args, "-a");
        let almost_all = utils::has_flag(args, "-A");
        let long = utils::has_flag(args, "-l");
        let human = utils::has_flag(args, "-h");
        let reverse = utils::has_flag(args, "-r");
        let sort_size = utils::has_flag(args, "-S");
        let sort_time = utils::has_flag(args, "-t");
        let one_per_line = utils::has_flag(args, "-1");
        let list_dirs = utils::has_flag(args, "-d");
        let recursive = utils::has_flag(args, "-R");
        let inode = utils::has_flag(args, "-i");
        let numeric_ids = utils::has_flag(args, "-n");
        let classify = utils::has_flag(args, "-F") || utils::has_flag(args, "-p");
        let no_sort = utils::has_flag(args, "-f");

        let targets = utils::positional_args(args, &[]);
        let paths: Vec<PathBuf> = if targets.is_empty() {
            vec![ctx.cwd.clone()]
        } else {
            targets.iter().map(|t| resolve(t, &ctx.cwd)).collect()
        };

        let multiple = paths.len() > 1;
        for path in &paths {
            if multiple {
                println!("{}:", path.display());
            }
            list_dir(
                path,
                list_dirs,
                show_all || no_sort,
                almost_all,
                long,
                human,
                reverse,
                sort_size,
                sort_time,
                one_per_line,
                inode,
                numeric_ids,
                classify,
                recursive,
                no_sort,
            )?;
            if multiple {
                println!();
            }
        }
        Ok(0)
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn list_dir(
    path: &Path,
    list_dirs: bool,
    show_hidden: bool,
    almost_all: bool,
    long: bool,
    human: bool,
    reverse: bool,
    sort_size: bool,
    sort_time: bool,
    one_per_line: bool,
    inode: bool,
    _numeric_ids: bool,
    classify: bool,
    recursive: bool,
    no_sort: bool,
) -> Result<(), ExecError> {
    if !path.exists() && !path.is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("ls: {}: No such file or directory", path.display()),
        )
        .into());
    }
    if list_dirs || !path.is_dir() {
        print_entry(path, long, human, inode, classify)?;
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(path)?
        .filter_map(std::result::Result::ok)
        .collect();

    // Filter hidden
    if !show_hidden && !almost_all {
        entries.retain(|e| !e.file_name().to_string_lossy().starts_with('.'));
    } else if almost_all {
        entries.retain(|e| {
            let n = e.file_name();
            n != "." && n != ".."
        });
    }

    // Sort
    if !no_sort {
        if sort_size {
            entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().map(|m| m.len()).unwrap_or(0)));
        } else if sort_time {
            entries.sort_by_key(|e| {
                std::cmp::Reverse(
                    e.metadata()
                        .and_then(|m| m.modified())
                        .map(|t| {
                            t.duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0)
                        })
                        .unwrap_or(0),
                )
            });
        } else {
            entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());
        }
        if reverse {
            entries.reverse();
        }
    }

    if long {
        print_long_header();
    }

    for entry in &entries {
        let ep = entry.path();
        if long {
            print_long_entry(&ep, human, inode, classify)?;
        } else if inode {
            let ino = get_inode(&ep);
            print!("{ino:8} ");
            print_name(&ep, classify);
            println!();
        } else if one_per_line || !long {
            print_name(&ep, classify);
            println!();
        }
    }

    if recursive {
        for entry in &entries {
            let ep = entry.path();
            if ep.is_dir() {
                println!("\n{}:", ep.display());
                list_dir(
                    &ep,
                    false,
                    show_hidden,
                    almost_all,
                    long,
                    human,
                    reverse,
                    sort_size,
                    sort_time,
                    one_per_line,
                    inode,
                    false,
                    classify,
                    recursive,
                    no_sort,
                )?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::fn_params_excessive_bools)]
fn print_entry(
    path: &Path,
    long: bool,
    human: bool,
    inode: bool,
    classify: bool,
) -> Result<(), ExecError> {
    if long {
        print_long_entry(path, human, inode, classify)
    } else {
        if inode {
            print!("{:8} ", get_inode(path));
        }
        print_name(path, classify);
        println!();
        Ok(())
    }
}

fn print_long_header() {
    // intentionally blank — ls -l doesn't print a header line
}

fn print_long_entry(
    path: &Path,
    human: bool,
    inode: bool,
    classify: bool,
) -> Result<(), ExecError> {
    let meta = std::fs::symlink_metadata(path)?;
    let is_dir = meta.is_dir();
    let is_link = meta.file_type().is_symlink();
    let size = meta.len();
    let mtime = meta.modified().map(utils::format_time).unwrap_or_default();
    let mode = get_mode(&meta);
    let perm = utils::format_mode(mode, is_dir, is_link);
    let nlinks = get_nlinks(&meta);
    let uid_s = get_uid_str(&meta);
    let gid_s = get_gid_str(&meta);
    let size_s = if human {
        utils::format_size_human(size)
    } else {
        size.to_string()
    };

    if inode {
        print!("{:8} ", get_inode(path));
    }
    print!("{perm} {nlinks:3} {uid_s:<8} {gid_s:<8} {size_s:>8} {mtime} ");
    print_name(path, classify);

    if is_link {
        if let Ok(target) = std::fs::read_link(path) {
            print!(" -> {}", target.display());
        }
    }
    println!();
    Ok(())
}

fn print_name(path: &Path, classify: bool) {
    let name = path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    if classify {
        let suffix = if path.is_dir() {
            "/"
        } else if is_executable(path) {
            "*"
        } else if path.is_symlink() {
            "@"
        } else {
            ""
        };
        print!("{name}{suffix}");
    } else {
        print!("{name}");
    }
}

#[cfg(unix)]
fn get_mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}
#[cfg(not(unix))]
fn get_mode(_meta: &std::fs::Metadata) -> u32 {
    0o644
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
fn get_uid_str(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    meta.uid().to_string()
}
#[cfg(not(unix))]
fn get_uid_str(_meta: &std::fs::Metadata) -> String {
    "user".into()
}

#[cfg(unix)]
fn get_gid_str(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt;
    meta.gid().to_string()
}
#[cfg(not(unix))]
fn get_gid_str(_meta: &std::fs::Metadata) -> String {
    "group".into()
}

#[cfg(unix)]
fn get_inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path)
        .map(|m| m.ino())
        .unwrap_or(0)
}
#[cfg(not(unix))]
fn get_inode(_path: &Path) -> u64 {
    0
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .map(|m| m.mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
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
    fn test_ls_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Ls.run(&[], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_ls_long() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Ls.run(&["-l".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_ls_all() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Ls.run(&["-a".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_ls_nonexistent_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/");
        assert!(Ls.run(&["/does/not/exist/xyz".into()], &mut ctx).is_err());
    }
}

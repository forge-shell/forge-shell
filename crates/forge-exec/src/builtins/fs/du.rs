use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Du;

impl BuiltinCommand for Du {
    fn name(&self) -> &'static str {
        "du"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let show_all = utils::has_flag(args, "-a");
        let grand_total = utils::has_flag(args, "-c");
        let human = utils::has_flag(args, "-h");
        let si = utils::has_flag(args, "--si");
        let mega = utils::has_flag(args, "-m");
        let summarize = utils::has_flag(args, "-s");
        let apparent = utils::has_flag(args, "-A") || utils::has_flag(args, "--apparent-size");
        let follow_links = utils::has_flag(args, "-L");
        let max_depth = if summarize {
            Some(0usize)
        } else {
            utils::flag_value(args, "-d")
                .or_else(|| utils::flag_value(args, "--max-depth"))
                .and_then(|v| v.parse().ok())
        };

        let block_size: u64 = if mega { 1024 * 1024 } else { 1024 };

        let targets = utils::positional_args(args, &["-d", "--max-depth"]);
        let paths: Vec<PathBuf> = if targets.is_empty() {
            vec![ctx.cwd.clone()]
        } else {
            targets.iter().map(|t| resolve(t, &ctx.cwd)).collect()
        };

        let mut total_bytes = 0u64;

        for path in &paths {
            let bytes = du_path(
                path,
                0,
                max_depth,
                show_all,
                apparent,
                follow_links,
                block_size,
                human,
                si,
            )?;
            total_bytes += bytes;
        }

        if grand_total {
            print_size(total_bytes, block_size, human, si);
            println!("\ttotal");
        }

        Ok(0)
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn du_path(
    path: &Path,
    depth: usize,
    max_depth: Option<usize>,
    show_all: bool,
    apparent: bool,
    follow_links: bool,
    block_size: u64,
    human: bool,
    si: bool,
) -> Result<u64, ExecError> {
    let meta = if follow_links {
        std::fs::metadata(path)?
    } else {
        std::fs::symlink_metadata(path)?
    };

    if meta.is_dir() {
        let mut dir_total = 0u64;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let sub = du_path(
                    &entry.path(),
                    depth + 1,
                    max_depth,
                    show_all,
                    apparent,
                    follow_links,
                    block_size,
                    human,
                    si,
                )?;
                dir_total += sub;
            }
        }
        if max_depth.is_none_or(|d| depth <= d) {
            print_size(dir_total, block_size, human, si);
            println!("\t{}", path.display());
        }
        Ok(dir_total)
    } else {
        let size = if apparent {
            meta.len()
        } else {
            get_blocks_bytes(&meta)
        };
        if show_all && max_depth.is_none_or(|d| depth <= d) {
            print_size(size, block_size, human, si);
            println!("\t{}", path.display());
        }
        Ok(size)
    }
}

fn print_size(bytes: u64, block_size: u64, human: bool, si: bool) {
    if human {
        print!("{}", utils::format_size_human(bytes));
    } else if si {
        print!("{}", utils::format_size_si(bytes));
    } else {
        let blocks = bytes.div_ceil(block_size);
        print!("{blocks}");
    }
}

#[cfg(unix)]
fn get_blocks_bytes(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    // blocks() returns 512-byte units
    meta.blocks() * 512
}
#[cfg(not(unix))]
fn get_blocks_bytes(meta: &std::fs::Metadata) -> u64 {
    meta.len()
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
    fn test_du_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Du.run(&[], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_du_summarize() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Du.run(&["-s".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_du_all() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"data").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Du.run(&["-a".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_du_human() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Du.run(&["-h".into()], &mut ctx).unwrap(), 0);
    }
}

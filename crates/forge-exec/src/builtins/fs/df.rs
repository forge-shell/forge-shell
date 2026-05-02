use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Df;

impl BuiltinCommand for Df {
    fn name(&self) -> &'static str {
        "df"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let human = utils::has_flag(args, "-h");
        let si = utils::has_flag(args, "-H") || utils::has_flag(args, "--si");
        let mega = utils::has_flag(args, "-m");
        let blocks = utils::has_flag(args, "-b");
        let total = utils::has_flag(args, "--total") || utils::has_flag(args, "-c");

        let block_size: u64 = if mega {
            1024 * 1024
        } else if blocks {
            512
        } else {
            1024
        };

        let targets = utils::positional_args(args, &["-T", "--type"]);
        let paths: Vec<PathBuf> = if targets.is_empty() {
            vec![ctx.cwd.clone()]
        } else {
            targets.iter().map(|t| resolve(t, &ctx.cwd)).collect()
        };

        println!(
            "{:<20} {:>12} {:>12} {:>12} {:>6} Mounted on",
            "Filesystem",
            size_hdr(block_size),
            "Used",
            "Avail",
            "Use%",
        );

        let mut grand_total = 0u64;
        let mut grand_used = 0u64;
        let mut grand_avail = 0u64;

        for path in &paths {
            let (fs_total, fs_avail, fs_name) = get_disk_info(path);
            let used = fs_total.saturating_sub(fs_avail);
            let use_pct = if fs_total > 0 {
                (used * 100) / fs_total
            } else {
                0
            };
            grand_total += fs_total;
            grand_used += used;
            grand_avail += fs_avail;
            println!(
                "{:<20} {:>12} {:>12} {:>12} {:>5}% {}",
                fs_name,
                fmt_size(fs_total, block_size, human, si),
                fmt_size(used, block_size, human, si),
                fmt_size(fs_avail, block_size, human, si),
                use_pct,
                path.display()
            );
        }

        if total {
            let grand_pct = if grand_total > 0 {
                (grand_used * 100) / grand_total
            } else {
                0
            };
            println!(
                "{:<20} {:>12} {:>12} {:>12} {:>5}% -",
                "total",
                fmt_size(grand_total, block_size, human, si),
                fmt_size(grand_used, block_size, human, si),
                fmt_size(grand_avail, block_size, human, si),
                grand_pct,
            );
        }

        Ok(0)
    }
}

fn size_hdr(block_size: u64) -> &'static str {
    if block_size == 512 {
        "512B-blocks"
    } else if block_size == 1024 * 1024 {
        "1M-blocks"
    } else {
        "1K-blocks"
    }
}

fn fmt_size(bytes: u64, block_size: u64, human: bool, si: bool) -> String {
    if human {
        utils::format_size_human(bytes)
    } else if si {
        utils::format_size_si(bytes)
    } else {
        let b = bytes.div_ceil(block_size);
        b.to_string()
    }
}

fn get_disk_info(path: &Path) -> (u64, u64, String) {
    let total = fs2::total_space(path).unwrap_or(0);
    let avail = fs2::available_space(path).unwrap_or(0);
    let fs_name = get_fs_name(path);
    (total, avail, fs_name)
}

fn get_fs_name(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

    #[test]
    fn test_df_basic() {
        let mut ctx = ShellContext::new();
        assert_eq!(Df.run(&[], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_df_human() {
        let mut ctx = ShellContext::new();
        assert_eq!(Df.run(&["-h".into()], &mut ctx).unwrap(), 0);
    }
}

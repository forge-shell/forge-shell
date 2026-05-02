use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Tree;

impl BuiltinCommand for Tree {
    fn name(&self) -> &'static str {
        "tree"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let show_hidden = utils::has_flag(args, "-a");
        let dirs_only = utils::has_flag(args, "-d");
        let full_path = utils::has_flag(args, "-f");
        let show_size = utils::has_flag(args, "-s");
        let human_size = utils::has_flag(args, "-h");
        let no_report = utils::has_flag(args, "--noreport");
        let max_depth = utils::flag_value(args, "-L")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(usize::MAX);

        let targets = utils::positional_args(args, &["-L"]);
        let root = if targets.is_empty() {
            ctx.cwd.clone()
        } else {
            resolve(targets[0], &ctx.cwd)
        };

        println!("{}", root.display());
        let (dirs, files) = print_tree(
            &root,
            "",
            0,
            max_depth,
            show_hidden,
            dirs_only,
            full_path,
            show_size,
            human_size,
        )?;
        if !no_report {
            println!(
                "\n{dirs} {}, {files} {}",
                plural(dirs, "directory", "directories"),
                plural(files, "file", "files")
            );
        }
        Ok(0)
    }
}

fn plural(n: usize, singular: &'static str, plural_form: &'static str) -> &'static str {
    if n == 1 { singular } else { plural_form }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn print_tree(
    dir: &Path,
    prefix: &str,
    depth: usize,
    max_depth: usize,
    show_hidden: bool,
    dirs_only: bool,
    full_path: bool,
    show_size: bool,
    human_size: bool,
) -> Result<(usize, usize), ExecError> {
    if depth >= max_depth {
        return Ok((0, 0));
    }

    let mut entries: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(std::result::Result::ok)
        .collect();

    if !show_hidden {
        entries.retain(|e| !e.file_name().to_string_lossy().starts_with('.'));
    }

    entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());

    if dirs_only {
        entries.retain(|e| e.path().is_dir());
    }

    let mut total_dirs = 0usize;
    let mut total_files = 0usize;

    let n = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let path = entry.path();
        let is_last = i + 1 == n;
        let connector = if is_last { "└── " } else { "├── " };
        let name = if full_path {
            path.display().to_string()
        } else {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        };

        let size_str = if show_size {
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            let s = if human_size {
                utils::format_size_human(size)
            } else {
                size.to_string()
            };
            format!("[{s}]  ")
        } else {
            String::new()
        };

        println!("{prefix}{connector}{size_str}{name}");

        if path.is_dir() {
            total_dirs += 1;
            let extension = if is_last { "    " } else { "│   " };
            let new_prefix = format!("{prefix}{extension}");
            let (d, f) = print_tree(
                &path,
                &new_prefix,
                depth + 1,
                max_depth,
                show_hidden,
                dirs_only,
                full_path,
                show_size,
                human_size,
            )?;
            total_dirs += d;
            total_files += f;
        } else {
            total_files += 1;
        }
    }
    Ok((total_dirs, total_files))
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
    fn test_tree_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Tree.run(&[], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_tree_depth_limit() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.txt"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Tree.run(&["-L".into(), "1".into()], &mut ctx).unwrap(), 0);
    }
}

use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub struct Wc;

impl BuiltinCommand for Wc {
    fn name(&self) -> &'static str {
        "wc"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let count_lines = utils::has_flag(args, "-l");
        let count_words = utils::has_flag(args, "-w");
        let count_bytes = utils::has_flag(args, "-c");
        let count_chars = utils::has_flag(args, "-m");
        let max_line_len = utils::has_flag(args, "-L");

        // Default: lines, words, bytes when no flag given
        let default_mode =
            !count_lines && !count_words && !count_bytes && !count_chars && !max_line_len;

        let targets = utils::positional_args(args, &[]);

        let mut total_lines = 0usize;
        let mut total_words = 0usize;
        let mut total_bytes = 0usize;
        let mut total_chars = 0usize;
        let mut total_max = 0usize;
        let multiple = targets.len() > 1;

        if targets.is_empty() {
            let stdin = std::io::stdin();
            let (lines, words, bytes, chars, max_len) = count(&mut stdin.lock())?;
            print_counts(
                lines,
                words,
                bytes,
                chars,
                max_len,
                None,
                default_mode,
                count_lines,
                count_words,
                count_bytes,
                count_chars,
                max_line_len,
            );
        } else {
            for t in &targets {
                let path = resolve(t, &ctx.cwd);
                let file = std::fs::File::open(&path)?;
                let (lines, words, bytes, chars, max_len) =
                    count(&mut std::io::BufReader::new(file))?;
                print_counts(
                    lines,
                    words,
                    bytes,
                    chars,
                    max_len,
                    Some(t),
                    default_mode,
                    count_lines,
                    count_words,
                    count_bytes,
                    count_chars,
                    max_line_len,
                );
                total_lines += lines;
                total_words += words;
                total_bytes += bytes;
                total_chars += chars;
                total_max = total_max.max(max_len);
            }
            if multiple {
                print_counts(
                    total_lines,
                    total_words,
                    total_bytes,
                    total_chars,
                    total_max,
                    Some("total"),
                    default_mode,
                    count_lines,
                    count_words,
                    count_bytes,
                    count_chars,
                    max_line_len,
                );
            }
        }
        Ok(0)
    }
}

fn count(reader: &mut dyn BufRead) -> Result<(usize, usize, usize, usize, usize), ExecError> {
    let mut lines = 0usize;
    let mut words = 0usize;
    let mut bytes = 0usize;
    let mut chars = 0usize;
    let mut max_line = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        lines += 1;
        bytes += n;
        chars += line.chars().count();
        words += line.split_whitespace().count();
        let line_len = line.trim_end_matches(['\n', '\r']).len();
        max_line = max_line.max(line_len);
    }
    Ok((lines, words, bytes, chars, max_line))
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn print_counts(
    lines: usize,
    words: usize,
    bytes: usize,
    chars: usize,
    max: usize,
    name: Option<&str>,
    default_mode: bool,
    cl: bool,
    cw: bool,
    cb: bool,
    cc: bool,
    cm: bool,
) {
    if default_mode {
        print!("{lines:>8} {words:>8} {bytes:>8}");
    } else {
        if cl {
            print!("{lines:>8}");
        }
        if cw {
            print!("{words:>8}");
        }
        if cb {
            print!("{bytes:>8}");
        }
        if cc {
            print!("{chars:>8}");
        }
        if cm {
            print!("{max:>8}");
        }
    }
    if let Some(n) = name {
        println!(" {n}");
    } else {
        println!();
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
    fn test_wc_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello world\nfoo bar\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Wc.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_wc_lines() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\nb\nc\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Wc.run(&["-l".into(), "f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_wc_words() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"one two three\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Wc.run(&["-w".into(), "f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_wc_missing_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/");
        assert!(Wc.run(&["__no_such__".into()], &mut ctx).is_err());
    }
}

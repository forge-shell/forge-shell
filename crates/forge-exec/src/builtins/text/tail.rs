use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::{BufRead, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct Tail;

impl BuiltinCommand for Tail {
    fn name(&self) -> &'static str {
        "tail"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let quiet = utils::has_flag(args, "-q")
            || utils::has_flag(args, "--quiet")
            || utils::has_flag(args, "--silent");
        let verbose = utils::has_flag(args, "-v") || utils::has_flag(args, "--verbose");
        let follow = utils::has_flag(args, "-f") || utils::has_flag(args, "-F");
        let reverse = utils::has_flag(args, "-r");

        let (byte_mode, count, from_start) = parse_count(args)?;
        let targets = utils::positional_args(args, &["-n", "-c", "-b"]);

        let multiple = targets.len() > 1;

        if targets.is_empty() {
            let stdin = std::io::stdin();
            output_tail(&mut stdin.lock(), count, byte_mode, from_start, reverse)?;
        } else {
            for (i, t) in targets.iter().enumerate() {
                let show_header = verbose || (multiple && !quiet);
                if show_header {
                    if i > 0 {
                        println!();
                    }
                    println!("==> {t} <==");
                }
                let path = resolve(t, &ctx.cwd);
                if *t == "-" {
                    let stdin = std::io::stdin();
                    output_tail(&mut stdin.lock(), count, byte_mode, from_start, reverse)?;
                } else {
                    let f = std::fs::File::open(&path)?;
                    output_tail(
                        &mut std::io::BufReader::new(f),
                        count,
                        byte_mode,
                        from_start,
                        reverse,
                    )?;
                }

                if follow {
                    follow_file(&path)?;
                }
            }
        }
        Ok(0)
    }
}

/// Returns `(byte_mode, count, from_start)`.
/// `from_start` is true for `+N` syntax.
fn parse_count(args: &[String]) -> Result<(bool, usize, bool), ExecError> {
    if let Some(v) = utils::flag_value(args, "-c") {
        let from_start = v.starts_with('+');
        let v2 = v.trim_start_matches('+').trim_start_matches('-');
        let n: usize = v2
            .parse()
            .map_err(|_| ExecError::InvalidArgument(format!("tail: invalid byte count: {v}")))?;
        return Ok((true, n, from_start));
    }
    if let Some(v) = utils::flag_value(args, "-b") {
        let n: usize = v.trim_start_matches(['+', '-']).parse().unwrap_or(1);
        return Ok((true, n * 512, v.starts_with('+')));
    }
    if let Some(v) = utils::flag_value(args, "-n") {
        let from_start = v.starts_with('+');
        let v2 = v.trim_start_matches('+').trim_start_matches('-');
        let n: usize = v2
            .parse()
            .map_err(|_| ExecError::InvalidArgument(format!("tail: invalid line count: {v}")))?;
        return Ok((false, n, from_start));
    }
    Ok((false, 10, false))
}

fn output_tail(
    reader: &mut dyn BufRead,
    count: usize,
    byte_mode: bool,
    from_start: bool,
    reverse: bool,
) -> Result<(), ExecError> {
    if byte_mode {
        let mut all = Vec::new();
        reader.read_to_end(&mut all)?;
        let slice = if from_start {
            let start = count.saturating_sub(1);
            if start < all.len() {
                &all[start..]
            } else {
                &[][..]
            }
        } else {
            let start = all.len().saturating_sub(count);
            &all[start..]
        };
        std::io::Write::write_all(&mut std::io::stdout(), slice)?;
        return Ok(());
    }

    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        lines.push(line.clone());
    }

    let output: Vec<&String> = if from_start {
        let start = count.saturating_sub(1);
        lines[start.min(lines.len())..].iter().collect()
    } else {
        let start = lines.len().saturating_sub(count);
        lines[start..].iter().collect()
    };

    if reverse {
        for l in output.iter().rev() {
            print!("{l}");
        }
    } else {
        for l in &output {
            print!("{l}");
        }
    }
    Ok(())
}

fn follow_file(path: &Path) -> Result<(), ExecError> {
    use std::thread::sleep;
    use std::time::Duration;
    let mut f = std::fs::File::open(path)?;
    f.seek(SeekFrom::End(0))?;
    // In test/non-interactive context, we only read once
    let mut buf = String::new();
    let n = std::io::BufReader::new(&f).read_line(&mut buf)?;
    if n > 0 {
        print!("{buf}");
    } else {
        sleep(Duration::from_millis(100));
    }
    Ok(())
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
    use std::fmt::Write;
    use tempfile::TempDir;

    fn ctx_in(tmp: &TempDir) -> ShellContext {
        let mut ctx = ShellContext::new();
        ctx.cwd = tmp.path().to_path_buf();
        ctx
    }

    #[test]
    fn test_tail_default() {
        let tmp = TempDir::new().unwrap();
        let content: String = (1..=20).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line{i}");
            s
        });
        std::fs::write(tmp.path().join("f.txt"), content.as_bytes()).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Tail.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_tail_n() {
        let tmp = TempDir::new().unwrap();
        let content: String = (1..=5).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line{i}");
            s
        });
        std::fs::write(tmp.path().join("f.txt"), content.as_bytes()).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Tail.run(&["-n".into(), "3".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_tail_from_start() {
        let tmp = TempDir::new().unwrap();
        let content: String = (1..=5).fold(String::new(), |mut s, i| {
            let _ = writeln!(s, "line{i}");
            s
        });
        std::fs::write(tmp.path().join("f.txt"), content.as_bytes()).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Tail.run(&["-n".into(), "+3".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_tail_missing_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/");
        assert!(Tail.run(&["__no_such__".into()], &mut ctx).is_err());
    }
}

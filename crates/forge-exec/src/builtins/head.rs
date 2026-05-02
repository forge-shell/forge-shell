use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub struct Head;

impl BuiltinCommand for Head {
    fn name(&self) -> &'static str {
        "head"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let quiet = utils::has_flag(args, "-q")
            || utils::has_flag(args, "--quiet")
            || utils::has_flag(args, "--silent");
        let verbose = utils::has_flag(args, "-v") || utils::has_flag(args, "--verbose");

        let (byte_mode, count) = parse_count(args)?;
        let targets = utils::positional_args(args, &["-n", "-c"]);

        let multiple = targets.len() > 1;

        if targets.is_empty() {
            let stdin = std::io::stdin();
            output_head(&mut stdin.lock(), count, byte_mode)?;
        } else {
            for (i, t) in targets.iter().enumerate() {
                let show_header = verbose || (multiple && !quiet);
                if show_header {
                    if i > 0 {
                        println!();
                    }
                    println!("==> {t} <==");
                }
                if *t == "-" {
                    let stdin = std::io::stdin();
                    output_head(&mut stdin.lock(), count, byte_mode)?;
                } else {
                    let path = resolve(t, &ctx.cwd);
                    let f = std::fs::File::open(&path)?;
                    output_head(&mut std::io::BufReader::new(f), count, byte_mode)?;
                }
            }
        }
        Ok(0)
    }
}

/// Returns `(byte_mode, count)`. Negative count means "all but last N".
fn parse_count(args: &[String]) -> Result<(bool, isize), ExecError> {
    if let Some(v) = utils::flag_value(args, "-c") {
        let n: isize = v
            .parse()
            .map_err(|_| ExecError::InvalidArgument(format!("head: invalid byte count: {v}")))?;
        return Ok((true, n));
    }
    if let Some(v) = utils::flag_value(args, "-n") {
        let n: isize = v
            .parse()
            .map_err(|_| ExecError::InvalidArgument(format!("head: invalid line count: {v}")))?;
        return Ok((false, n));
    }
    Ok((false, 10))
}

#[allow(clippy::cast_sign_loss)]
fn output_head(reader: &mut dyn BufRead, count: isize, byte_mode: bool) -> Result<(), ExecError> {
    if byte_mode {
        if count >= 0 {
            let mut buf = vec![0u8; count as usize];
            let n = reader.read(&mut buf)?;
            std::io::Write::write_all(&mut std::io::stdout(), &buf[..n])?;
        } else {
            // All but last |count| bytes
            let mut all = Vec::new();
            reader.read_to_end(&mut all)?;
            let skip = (-count) as usize;
            if all.len() > skip {
                std::io::Write::write_all(&mut std::io::stdout(), &all[..all.len() - skip])?;
            }
        }
    } else if count >= 0 {
        let mut remaining = count as usize;
        let mut line = String::new();
        while remaining > 0 {
            line.clear();
            let n = reader.read_line(&mut line)?;
            if n == 0 {
                break;
            }
            print!("{line}");
            remaining -= 1;
        }
    } else {
        // All but last |count| lines
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
        let skip = (-count) as usize;
        let take = lines.len().saturating_sub(skip);
        for l in &lines[..take] {
            print!("{l}");
        }
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
    use tempfile::TempDir;

    fn ctx_in(tmp: &TempDir) -> ShellContext {
        let mut ctx = ShellContext::new();
        ctx.cwd = tmp.path().to_path_buf();
        ctx
    }

    #[test]
    fn test_head_default() {
        let tmp = TempDir::new().unwrap();
        let content: String = (1..=20).map(|i| format!("line{i}\n")).collect();
        std::fs::write(tmp.path().join("f.txt"), content.as_bytes()).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Head.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_head_n() {
        let tmp = TempDir::new().unwrap();
        let content: String = (1..=5).map(|i| format!("line{i}\n")).collect();
        std::fs::write(tmp.path().join("f.txt"), content.as_bytes()).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Head.run(&["-n".into(), "3".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_head_missing_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/");
        assert!(Head.run(&["__no_such__".into()], &mut ctx).is_err());
    }
}

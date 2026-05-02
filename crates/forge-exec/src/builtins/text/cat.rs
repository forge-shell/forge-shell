use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub struct Cat;

impl BuiltinCommand for Cat {
    fn name(&self) -> &'static str {
        "cat"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let number_all = utils::has_flag(args, "-n");
        let number_nblank = utils::has_flag(args, "-b");
        let squeeze = utils::has_flag(args, "-s");
        let show_nonprint = utils::has_flag(args, "-v")
            || utils::has_flag(args, "-e")
            || utils::has_flag(args, "-t")
            || utils::has_flag(args, "-A");
        let show_ends = utils::has_flag(args, "-e")
            || utils::has_flag(args, "-E")
            || utils::has_flag(args, "-A");
        let show_tabs = utils::has_flag(args, "-t")
            || utils::has_flag(args, "-T")
            || utils::has_flag(args, "-A");

        let targets = utils::positional_args(args, &[]);

        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let mut line_num = 0usize;
        let mut prev_blank = false;

        let process = |reader: &mut dyn BufRead,
                       line_num: &mut usize,
                       prev_blank: &mut bool,
                       out: &mut dyn Write|
         -> Result<(), ExecError> {
            let mut buf = String::new();
            loop {
                buf.clear();
                let n = reader.read_line(&mut buf)?;
                if n == 0 {
                    break;
                }

                let is_blank = buf.trim_end_matches(['\n', '\r']).is_empty();

                if squeeze && is_blank && *prev_blank {
                    continue;
                }
                *prev_blank = is_blank;

                let mut line = buf.trim_end_matches(['\n', '\r']).to_string();
                let newline = if buf.ends_with('\n') { "\n" } else { "" };

                if show_tabs {
                    line = line.replace('\t', "^I");
                }
                if show_nonprint {
                    line = render_nonprint(&line);
                }

                let suffix = if show_ends { "$" } else { "" };

                if (number_nblank && !is_blank) || (number_all && !number_nblank) {
                    *line_num += 1;
                    write!(out, "{:>6}\t{line}{suffix}{newline}", *line_num)?;
                } else {
                    write!(out, "{line}{suffix}{newline}")?;
                }
            }
            Ok(())
        };

        if targets.is_empty() {
            let stdin = std::io::stdin();
            process(&mut stdin.lock(), &mut line_num, &mut prev_blank, &mut out)?;
        } else {
            for t in &targets {
                if *t == "-" {
                    let stdin = std::io::stdin();
                    process(&mut stdin.lock(), &mut line_num, &mut prev_blank, &mut out)?;
                } else {
                    let path = resolve(t, &ctx.cwd);
                    let f = std::fs::File::open(&path)?;
                    process(
                        &mut std::io::BufReader::new(f),
                        &mut line_num,
                        &mut prev_blank,
                        &mut out,
                    )?;
                }
            }
        }
        Ok(0)
    }
}

#[allow(clippy::cast_possible_truncation)]
fn render_nonprint(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let b = c as u32;
        if b < 0x20 && c != '\t' {
            out.push('^');
            out.push((b as u8 + 0x40) as char);
        } else if b == 0x7f {
            out.push_str("^?");
        } else if (0x80..0xa0).contains(&b) {
            out.push_str("M-^");
            out.push(((b - 0x80 + 0x40) as u8) as char);
        } else if b >= 0xa0 {
            out.push_str("M-");
            out.push(char::from_u32(b - 0x80).unwrap_or('?'));
        } else {
            out.push(c);
        }
    }
    out
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
    fn test_cat_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello\nworld\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Cat.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_cat_number_lines() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\nb\nc\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Cat.run(&["-n".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_cat_squeeze() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\n\n\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Cat.run(&["-s".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_cat_missing_errors() {
        let mut ctx = ShellContext::new();
        ctx.cwd = std::path::PathBuf::from("/");
        assert!(Cat.run(&["__no_such_file__".into()], &mut ctx).is_err());
    }
}

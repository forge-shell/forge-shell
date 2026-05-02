use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub struct Uniq;

impl BuiltinCommand for Uniq {
    fn name(&self) -> &'static str {
        "uniq"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let count = utils::has_flag(args, "-c");
        let only_dup = utils::has_flag(args, "-d");
        let all_dup = utils::has_flag(args, "-D");
        let only_uniq = utils::has_flag(args, "-u");
        let case_insens = utils::has_flag(args, "-i");
        let skip_fields = utils::flag_value(args, "-f")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let skip_chars = utils::flag_value(args, "-s")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let max_chars = utils::flag_value(args, "-w").and_then(|v| v.parse::<usize>().ok());

        let targets = utils::positional_args(args, &["-f", "-s", "-w"]);

        let mut lines: Vec<String> = Vec::new();
        let read_lines =
            |reader: &mut dyn BufRead, out: &mut Vec<String>| -> Result<(), ExecError> {
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line)? == 0 {
                        break;
                    }
                    out.push(line.trim_end_matches(['\n', '\r']).to_string());
                }
                Ok(())
            };

        if targets.is_empty() {
            let stdin = std::io::stdin();
            read_lines(&mut stdin.lock(), &mut lines)?;
        } else {
            let path = resolve(targets[0], &ctx.cwd);
            let f = std::fs::File::open(&path)?;
            read_lines(&mut std::io::BufReader::new(f), &mut lines)?;
        }

        let key_of = |line: &str| -> String {
            let mut s = line;
            // Skip fields
            let mut remaining = skip_fields;
            while remaining > 0 {
                s = s.trim_start();
                s = s.trim_start_matches(|c: char| !c.is_whitespace());
                remaining -= 1;
            }
            // Skip chars
            let s = &s[skip_chars.min(s.len())..];
            // Max chars
            let s = if let Some(m) = max_chars {
                &s[..m.min(s.len())]
            } else {
                s
            };
            if case_insens {
                s.to_lowercase()
            } else {
                s.to_string()
            }
        };

        let stdout = std::io::stdout();
        let mut out = stdout.lock();

        // Group consecutive equal lines
        let mut i = 0;
        while i < lines.len() {
            let key = key_of(&lines[i]);
            let mut j = i + 1;
            while j < lines.len() && key_of(&lines[j]) == key {
                j += 1;
            }
            let run_len = j - i;

            let emit = if only_uniq {
                run_len == 1
            } else if only_dup || all_dup {
                run_len > 1
            } else {
                true
            };

            if emit {
                if all_dup {
                    for line in &lines[i..j] {
                        if count {
                            writeln!(out, "{run_len:>7} {line}")?;
                        } else {
                            writeln!(out, "{line}")?;
                        }
                    }
                } else if count {
                    let line = &lines[i];
                    writeln!(out, "{run_len:>7} {line}")?;
                } else {
                    writeln!(out, "{}", lines[i])?;
                }
            }
            i = j;
        }

        // Output to file if second positional arg given
        Ok(0)
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
    fn test_uniq_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\nb\nc\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Uniq.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_uniq_count() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Uniq.run(&["-c".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_uniq_only_dups() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Uniq.run(&["-d".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_uniq_only_unique() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Uniq.run(&["-u".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }
}

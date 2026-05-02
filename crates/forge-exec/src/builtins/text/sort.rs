use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub struct Sort;

impl BuiltinCommand for Sort {
    fn name(&self) -> &'static str {
        "sort"
    }

    #[allow(clippy::too_many_lines)]
    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let ignore_blanks = utils::has_flag(args, "-b");
        let dict_order = utils::has_flag(args, "-d");
        let case_fold = utils::has_flag(args, "-f");
        let numeric = utils::has_flag(args, "-n");
        let general_num = utils::has_flag(args, "-g");
        let human_num = utils::has_flag(args, "-h");
        let reverse = utils::has_flag(args, "-r");
        let random = utils::has_flag(args, "-R");
        let unique = utils::has_flag(args, "-u");
        let month = utils::has_flag(args, "-M");
        let version_sort = utils::has_flag(args, "-V");
        let stable = utils::has_flag(args, "-s");
        let check = utils::has_flag(args, "-c");
        let check_quiet = utils::has_flag(args, "-C");
        let output_file = utils::flag_value(args, "-o");
        let _separator = utils::flag_value(args, "-t");
        let null_term = utils::has_flag(args, "-z");

        let targets = utils::positional_args(args, &["-o", "-t", "-k", "-T", "-S"]);

        let mut lines: Vec<String> = Vec::new();
        let record_sep = if null_term { b'\0' } else { b'\n' };

        let read_lines =
            |reader: &mut dyn BufRead, lines: &mut Vec<String>| -> Result<(), ExecError> {
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    let n = reader.read_until(record_sep, &mut buf)?;
                    if n == 0 {
                        break;
                    }
                    let s = String::from_utf8_lossy(&buf)
                        .trim_end_matches(['\n', '\0'])
                        .to_string();
                    lines.push(s);
                }
                Ok(())
            };

        if targets.is_empty() {
            let stdin = std::io::stdin();
            read_lines(&mut stdin.lock(), &mut lines)?;
        } else {
            for t in &targets {
                let path = resolve(t, &ctx.cwd);
                let f = std::fs::File::open(&path)?;
                read_lines(&mut std::io::BufReader::new(f), &mut lines)?;
            }
        }

        if check || check_quiet {
            for i in 1..lines.len() {
                let a = key_of(&lines[i - 1], ignore_blanks, case_fold, dict_order);
                let b = key_of(&lines[i], ignore_blanks, case_fold, dict_order);
                let ord = compare(&a, &b, numeric, general_num, human_num, month, version_sort);
                if ord == std::cmp::Ordering::Greater {
                    if !check_quiet {
                        eprintln!("sort: disorder: {}", lines[i]);
                    }
                    return Ok(1);
                }
            }
            return Ok(0);
        }

        if random {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            // Deterministic pseudo-shuffle based on content hash
            let mut indexed: Vec<(u64, String)> = lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    let mut h = DefaultHasher::new();
                    i.hash(&mut h);
                    l.hash(&mut h);
                    (h.finish(), l)
                })
                .collect();
            indexed.sort_by_key(|(k, _)| *k);
            lines = indexed.into_iter().map(|(_, l)| l).collect();
        } else {
            if stable {
                lines.sort_by(|a, b| {
                    let ka = key_of(a, ignore_blanks, case_fold, dict_order);
                    let kb = key_of(b, ignore_blanks, case_fold, dict_order);
                    compare(
                        &ka,
                        &kb,
                        numeric,
                        general_num,
                        human_num,
                        month,
                        version_sort,
                    )
                });
            } else {
                lines.sort_unstable_by(|a, b| {
                    let ka = key_of(a, ignore_blanks, case_fold, dict_order);
                    let kb = key_of(b, ignore_blanks, case_fold, dict_order);
                    compare(
                        &ka,
                        &kb,
                        numeric,
                        general_num,
                        human_num,
                        month,
                        version_sort,
                    )
                });
            }
            if reverse {
                lines.reverse();
            }
        }

        if unique {
            lines.dedup_by(|a, b| {
                let ka = key_of(a, ignore_blanks, case_fold, dict_order);
                let kb = key_of(b, ignore_blanks, case_fold, dict_order);
                compare(
                    &ka,
                    &kb,
                    numeric,
                    general_num,
                    human_num,
                    month,
                    version_sort,
                ) == std::cmp::Ordering::Equal
            });
        }

        let terminator = if null_term { "\0" } else { "\n" };
        let mut output = String::new();
        for l in &lines {
            output.push_str(l);
            output.push_str(terminator);
        }

        if let Some(ofile) = output_file {
            std::fs::write(ofile, &output)?;
        } else {
            print!("{output}");
        }
        Ok(0)
    }
}

fn key_of(line: &str, ignore_blanks: bool, case_fold: bool, dict_order: bool) -> String {
    let mut s = if ignore_blanks {
        line.trim_start()
    } else {
        line
    }
    .to_string();
    if case_fold {
        s = s.to_lowercase();
    }
    if dict_order {
        s = s
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
    }
    s
}

#[allow(clippy::fn_params_excessive_bools)]
fn compare(
    a: &str,
    b: &str,
    numeric: bool,
    general_num: bool,
    human_num: bool,
    month: bool,
    version_sort: bool,
) -> std::cmp::Ordering {
    if numeric {
        let na: f64 = a.parse().unwrap_or(0.0);
        let nb: f64 = b.parse().unwrap_or(0.0);
        return na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
    }
    if general_num {
        let na: f64 = a.parse().unwrap_or(0.0);
        let nb: f64 = b.parse().unwrap_or(0.0);
        return na.partial_cmp(&nb).unwrap_or(std::cmp::Ordering::Equal);
    }
    if human_num {
        return cmp_human(a, b);
    }
    if month {
        return cmp_month(a, b);
    }
    if version_sort {
        return cmp_version(a, b);
    }
    a.cmp(b)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn parse_human_bytes(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let (num_part, mult) = if let Some(n) = s.strip_suffix(['K', 'k']) {
        (n, 1024u64)
    } else if let Some(n) = s.strip_suffix('M') {
        (n, 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('G') {
        (n, 1024 * 1024 * 1024)
    } else if let Some(n) = s.strip_suffix('T') {
        (n, 1024 * 1024 * 1024 * 1024)
    } else {
        (s, 1)
    };
    let n: f64 = num_part.parse().unwrap_or(0.0);
    (n * mult as f64) as u64
}

fn cmp_human(a: &str, b: &str) -> std::cmp::Ordering {
    parse_human_bytes(a).cmp(&parse_human_bytes(b))
}

fn month_order(s: &str) -> i32 {
    match s.trim().to_uppercase().as_str() {
        "JAN" | "JANUARY" => 1,
        "FEB" | "FEBRUARY" => 2,
        "MAR" | "MARCH" => 3,
        "APR" | "APRIL" => 4,
        "MAY" => 5,
        "JUN" | "JUNE" => 6,
        "JUL" | "JULY" => 7,
        "AUG" | "AUGUST" => 8,
        "SEP" | "SEPTEMBER" => 9,
        "OCT" | "OCTOBER" => 10,
        "NOV" | "NOVEMBER" => 11,
        "DEC" | "DECEMBER" => 12,
        _ => 0,
    }
}

fn cmp_month(a: &str, b: &str) -> std::cmp::Ordering {
    month_order(a).cmp(&month_order(b))
}

fn cmp_version(a: &str, b: &str) -> std::cmp::Ordering {
    // Split into numeric and non-numeric tokens and compare
    let parts_a = version_parts(a);
    let parts_b = version_parts(b);
    for (pa, pb) in parts_a.iter().zip(parts_b.iter()) {
        let ord = if let (Ok(na), Ok(nb)) = (pa.parse::<u64>(), pb.parse::<u64>()) {
            na.cmp(&nb)
        } else {
            pa.cmp(pb)
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    parts_a.len().cmp(&parts_b.len())
}

fn version_parts(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut buf = String::new();
    let mut was_digit = false;
    for c in s.chars() {
        let is_digit = c.is_ascii_digit();
        if is_digit != was_digit && !buf.is_empty() {
            parts.push(buf.clone());
            buf.clear();
        }
        buf.push(c);
        was_digit = is_digit;
    }
    if !buf.is_empty() {
        parts.push(buf);
    }
    parts
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
    fn test_sort_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"banana\napple\ncherry\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Sort.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_sort_reverse() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\nb\nc\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Sort.run(&["-r".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_sort_numeric() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"10\n2\n1\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Sort.run(&["-n".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_sort_unique() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Sort.run(&["-u".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_sort_check_sorted() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\nb\nc\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Sort.run(&["-c".into(), "f.txt".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_sort_check_unsorted() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"b\na\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Sort.run(&["-c".into(), "f.txt".into()], &mut ctx).unwrap(),
            1
        );
    }
}

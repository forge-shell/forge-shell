use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};

pub struct Diff;

impl BuiltinCommand for Diff {
    fn name(&self) -> &'static str {
        "diff"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let _unified = utils::has_flag(args, "-u");
        let context = utils::has_flag(args, "-c");
        let brief = utils::has_flag(args, "-q") || utils::has_flag(args, "--brief");
        let report_same = utils::has_flag(args, "-s");
        let ignore_case = utils::has_flag(args, "-i");
        let ignore_blank_lines = utils::has_flag(args, "-B");
        let ignore_ws = utils::has_flag(args, "-w");
        let ignore_trail = utils::has_flag(args, "-b") || utils::has_flag(args, "-Z");
        let _treat_text = utils::has_flag(args, "-a");
        let ctx_lines = utils::flag_value(args, "-U")
            .or_else(|| utils::flag_value(args, "-C"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(3);

        let targets = utils::positional_args(args, &["-U", "-C"]);
        if targets.len() < 2 {
            return Err(ExecError::InvalidArgument(
                "diff: missing operand after ...".into(),
            ));
        }

        let path1 = resolve(targets[0], &ctx.cwd);
        let path2 = resolve(targets[1], &ctx.cwd);

        let read = |path: &Path| -> Result<Vec<String>, ExecError> {
            if path.as_os_str() == "-" {
                use std::io::BufRead;
                let stdin = std::io::stdin();
                let mut lines = Vec::new();
                let mut line = String::new();
                loop {
                    line.clear();
                    if stdin.lock().read_line(&mut line)? == 0 {
                        break;
                    }
                    lines.push(line.clone());
                }
                Ok(lines)
            } else {
                Ok(std::fs::read_to_string(path)?
                    .lines()
                    .map(std::string::ToString::to_string)
                    .collect())
            }
        };

        let normalize = |lines: Vec<String>| -> Vec<String> {
            lines
                .into_iter()
                .map(|mut l| {
                    if ignore_case {
                        l = l.to_lowercase();
                    }
                    if ignore_ws {
                        l = l.split_whitespace().collect::<Vec<_>>().join(" ");
                    }
                    if ignore_trail {
                        l = l.trim_end().to_string();
                    }
                    l
                })
                .filter(|l| !(ignore_blank_lines && l.is_empty()))
                .collect()
        };

        let lines1_raw = read(&path1)?;
        let lines2_raw = read(&path2)?;
        let lines1 = normalize(lines1_raw.clone());
        let lines2 = normalize(lines2_raw.clone());

        if lines1 == lines2 {
            if report_same {
                println!(
                    "Files {} and {} are identical",
                    path1.display(),
                    path2.display()
                );
            }
            return Ok(0);
        }

        if brief {
            println!("Files {} and {} differ", path1.display(), path2.display());
            return Ok(1);
        }

        // Myers diff
        let hunks = myers_diff(&lines1, &lines2);

        if context {
            print_context_diff(&hunks, &lines1, &lines2, &path1, &path2, ctx_lines);
        } else {
            // Default: unified diff
            print_unified_diff(&hunks, &lines1, &lines2, &path1, &path2, ctx_lines);
        }

        Ok(1)
    }
}

/// Edit operation
#[derive(Debug, Clone, PartialEq)]
enum Edit {
    Keep(usize, usize), // (i in a, j in b)
    Delete(usize),      // i in a
    Insert(usize),      // j in b
}

#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]
fn myers_diff(a: &[String], b: &[String]) -> Vec<Edit> {
    let n = a.len();
    let m = b.len();
    let max = n + m;
    if max == 0 {
        return vec![];
    }

    let mut v: Vec<i64> = vec![0; 2 * max + 1];
    let offset = max as i64;
    let mut trace: Vec<Vec<i64>> = Vec::new();

    'outer: for d in 0..=(max as i64) {
        trace.push(v.clone());
        let mut k = -d;
        while k <= d {
            let idx = (k + offset) as usize;
            let mut x = if k == -d
                || (k != d && v[(k - 1 + offset) as usize] < v[(k + 1 + offset) as usize])
            {
                v[(k + 1 + offset) as usize]
            } else {
                v[(k - 1 + offset) as usize] + 1
            };
            let mut y = x - k;
            while x < n as i64 && y < m as i64 && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[idx] = x;
            if x >= n as i64 && y >= m as i64 {
                break 'outer;
            }
            k += 2;
        }
    }

    // Backtrack
    let mut edits = Vec::new();
    let mut x = n as i64;
    let mut y = m as i64;

    for d in (0..trace.len() as i64).rev() {
        let v_prev = &trace[d as usize];
        let k = x - y;
        let _ = (k + offset) as usize;
        let prev_k = if k == -d
            || (k != d && v_prev[(k - 1 + offset) as usize] < v_prev[(k + 1 + offset) as usize])
        {
            k + 1
        } else {
            k - 1
        };
        let prev_x = v_prev[(prev_k + offset) as usize];
        let prev_y = prev_x - prev_k;

        while x > prev_x + 1 && y > prev_y + 1 {
            edits.push(Edit::Keep((x - 1) as usize, (y - 1) as usize));
            x -= 1;
            y -= 1;
        }
        if d > 0 {
            if x == prev_x + 1 {
                edits.push(Edit::Delete((x - 1) as usize));
                x = prev_x;
            } else {
                edits.push(Edit::Insert((y - 1) as usize));
                y = prev_y;
            }
        }
        while x > prev_x && y > prev_y {
            edits.push(Edit::Keep((x - 1) as usize, (y - 1) as usize));
            x -= 1;
            y -= 1;
        }
    }
    edits.reverse();
    edits
}

fn print_unified_diff(
    edits: &[Edit],
    a: &[String],
    b: &[String],
    path1: &Path,
    path2: &Path,
    ctx: usize,
) {
    println!("--- {}", path1.display());
    println!("+++ {}", path2.display());

    // Group edits into hunks with context
    let changes: Vec<usize> = edits
        .iter()
        .enumerate()
        .filter(|(_, e)| !matches!(e, Edit::Keep(..)))
        .map(|(i, _)| i)
        .collect();

    if changes.is_empty() {
        return;
    }

    let mut hunk_starts: Vec<usize> = Vec::new();
    let mut prev_end: Option<usize> = None;
    for &ci in &changes {
        let hunk_start = ci.saturating_sub(ctx);
        if let Some(pe) = prev_end {
            if hunk_start <= pe + ctx * 2 {
                // Extend current hunk
                *hunk_starts.last_mut().unwrap() = (*hunk_starts.last().unwrap()).min(hunk_start);
            } else {
                hunk_starts.push(hunk_start);
            }
        } else {
            hunk_starts.push(hunk_start);
        }
        prev_end = Some((ci + ctx + 1).min(edits.len()));
    }

    for &hstart in &hunk_starts {
        let hend = {
            let last_change = changes
                .iter()
                .rev()
                .find(|&&ci| ci >= hstart)
                .copied()
                .unwrap_or(hstart);
            (last_change + ctx + 1).min(edits.len())
        };

        let slice = &edits[hstart..hend];

        let a_start = slice
            .iter()
            .find_map(|e| match e {
                Edit::Keep(i, _) | Edit::Delete(i) => Some(*i + 1),
                Edit::Insert(_) => None,
            })
            .unwrap_or(1);
        let b_start = slice
            .iter()
            .find_map(|e| match e {
                Edit::Keep(_, j) | Edit::Insert(j) => Some(*j + 1),
                Edit::Delete(_) => None,
            })
            .unwrap_or(1);
        let a_len = slice
            .iter()
            .filter(|e| !matches!(e, Edit::Insert(_)))
            .count();
        let b_len = slice
            .iter()
            .filter(|e| !matches!(e, Edit::Delete(_)))
            .count();

        println!("@@ -{a_start},{a_len} +{b_start},{b_len} @@");
        for edit in slice {
            match edit {
                Edit::Keep(i, _) => println!(" {}", a[*i]),
                Edit::Delete(i) => println!("-{}", a[*i]),
                Edit::Insert(j) => println!("+{}", b[*j]),
            }
        }
    }
}

fn print_context_diff(
    edits: &[Edit],
    a: &[String],
    b: &[String],
    path1: &Path,
    path2: &Path,
    ctx: usize,
) {
    println!("*** {}", path1.display());
    println!("--- {}", path2.display());

    let mut i = 0;
    while i < edits.len() {
        if matches!(&edits[i], Edit::Keep(..)) {
            i += 1;
            continue;
        }
        let start = i.saturating_sub(ctx);
        let mut end = i;
        while end < edits.len() && (!matches!(&edits[end], Edit::Keep(..)) || end < i + ctx) {
            end += 1;
        }
        end = (end + ctx).min(edits.len());

        let slice = &edits[start..end];
        println!("***************");
        let a_lines: Vec<_> = slice
            .iter()
            .filter(|e| !matches!(e, Edit::Insert(_)))
            .collect();
        println!("*** {},{} ****", start + 1, start + a_lines.len());
        for e in slice {
            match e {
                Edit::Keep(ai, _) => println!("  {}", a[*ai]),
                Edit::Delete(ai) => println!("- {}", a[*ai]),
                Edit::Insert(_) => {}
            }
        }
        let b_lines: Vec<_> = slice
            .iter()
            .filter(|e| !matches!(e, Edit::Delete(_)))
            .collect();
        println!("--- {},{} ----", start + 1, start + b_lines.len());
        for e in slice {
            match e {
                Edit::Keep(_, bi) => println!("  {}", b[*bi]),
                Edit::Insert(bi) => println!("+ {}", b[*bi]),
                Edit::Delete(_) => {}
            }
        }
        i = end;
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
    fn test_diff_identical() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"hello\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Diff.run(&["a.txt".into(), "b.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_diff_different() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"hello\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"world\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Diff.run(&["a.txt".into(), "b.txt".into()], &mut ctx)
                .unwrap(),
            1
        );
    }

    #[test]
    fn test_diff_brief() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"a\n").unwrap();
        std::fs::write(tmp.path().join("b.txt"), b"b\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Diff.run(&["-q".into(), "a.txt".into(), "b.txt".into()], &mut ctx)
                .unwrap(),
            1
        );
    }

    #[test]
    fn test_diff_missing_args_errors() {
        let mut ctx = ShellContext::new();
        assert!(Diff.run(&["a.txt".into()], &mut ctx).is_err());
    }
}

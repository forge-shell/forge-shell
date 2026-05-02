use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::BufRead;
use std::path::{Path, PathBuf};

pub struct Grep;

impl BuiltinCommand for Grep {
    fn name(&self) -> &'static str {
        "grep"
    }

    #[allow(clippy::too_many_lines)]
    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let extended = utils::has_flag(args, "-E");
        let fixed = utils::has_flag(args, "-F");
        let ignore_case = utils::has_flag(args, "-i");
        let invert = utils::has_flag(args, "-v");
        let whole_word = utils::has_flag(args, "-w");
        let whole_line = utils::has_flag(args, "-x");
        let count_only = utils::has_flag(args, "-c");
        let files_with = utils::has_flag(args, "-l");
        let files_without = utils::has_flag(args, "-L");
        let line_num = utils::has_flag(args, "-n");
        let no_fname = utils::has_flag(args, "-h");
        let with_fname = utils::has_flag(args, "-H");
        let only_match = utils::has_flag(args, "-o");
        let quiet = utils::has_flag(args, "-q");
        let suppress_err = utils::has_flag(args, "-s");
        let recursive = utils::has_flag(args, "-r") || utils::has_flag(args, "-R");
        let null_term = utils::has_flag(args, "-z");
        let max_count = utils::flag_value(args, "-m").and_then(|v| v.parse::<usize>().ok());
        let before_ctx = utils::flag_value(args, "-B")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let after_ctx = utils::flag_value(args, "-A")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let ctx_lines = utils::flag_value(args, "-C").and_then(|v| v.parse::<usize>().ok());
        let before_ctx = ctx_lines.unwrap_or(before_ctx);
        let after_ctx = ctx_lines.unwrap_or(after_ctx);

        let include_glob = utils::flag_value(args, "--include");
        let exclude_glob = utils::flag_value(args, "--exclude");

        // Collect explicit -e patterns
        let mut patterns: Vec<String> = args
            .windows(2)
            .filter(|w| w[0] == "-e")
            .map(|w| w[1].clone())
            .collect();

        // Pattern from -f file
        if let Some(pfile) = utils::flag_value(args, "-f") {
            let content = std::fs::read_to_string(pfile)?;
            for line in content.lines() {
                patterns.push(line.to_string());
            }
        }

        let targets = utils::positional_args(args, &["-e", "-f", "-m", "-B", "-A", "-C"]);

        // If no -e patterns yet, first positional is the pattern
        if patterns.is_empty() {
            if targets.is_empty() {
                return Err(ExecError::InvalidArgument("grep: missing pattern".into()));
            }
            patterns.push(targets[0].to_string());
        }

        let file_args: Vec<&str> =
            if patterns.len() == 1 && !args.iter().any(|a| a == "-e" || a == "-f") {
                targets[1..].to_vec()
            } else {
                targets.clone()
            };

        // Build regex matchers
        let matchers = build_matchers(
            &patterns,
            extended,
            fixed,
            ignore_case,
            whole_word,
            whole_line,
        )?;

        // Expand directories recursively
        let mut files: Vec<PathBuf> = Vec::new();
        if file_args.is_empty() {
            if recursive {
                collect_files(&ctx.cwd, &mut files, include_glob, exclude_glob);
            }
            // else stdin
        } else {
            for f in &file_args {
                let path = resolve(f, &ctx.cwd);
                if path.is_dir() && recursive {
                    collect_files(&path, &mut files, include_glob, exclude_glob);
                } else {
                    files.push(path);
                }
            }
        }

        let show_filename = with_fname || (!no_fname && files.len() > 1);
        let mut any_match = false;

        if files.is_empty() {
            // Read stdin
            let stdin = std::io::stdin();
            let matched = grep_reader(
                &mut stdin.lock(),
                None,
                &matchers,
                invert,
                count_only,
                files_with,
                files_without,
                line_num,
                show_filename,
                only_match,
                quiet,
                max_count,
                before_ctx,
                after_ctx,
                null_term,
            )?;
            if matched {
                any_match = true;
            }
        } else {
            for fpath in &files {
                let display = fpath.to_string_lossy().to_string();
                let f = match std::fs::File::open(fpath) {
                    Ok(f) => f,
                    Err(e) => {
                        if !suppress_err {
                            eprintln!("grep: {display}: {e}");
                        }
                        continue;
                    }
                };
                let matched = grep_reader(
                    &mut std::io::BufReader::new(f),
                    Some(&display),
                    &matchers,
                    invert,
                    count_only,
                    files_with,
                    files_without,
                    line_num,
                    show_filename,
                    only_match,
                    quiet,
                    max_count,
                    before_ctx,
                    after_ctx,
                    null_term,
                )?;
                if matched {
                    any_match = true;
                }
                if quiet && any_match {
                    break;
                }
            }
        }

        Ok(i32::from(!any_match))
    }
}

enum Matcher {
    Fixed(String, bool), // needle, case_insensitive
    Regex(regex::Regex),
}

impl Matcher {
    fn is_match(&self, haystack: &str) -> bool {
        match self {
            Matcher::Fixed(needle, ci) => {
                if *ci {
                    haystack.to_lowercase().contains(&needle.to_lowercase())
                } else {
                    haystack.contains(needle.as_str())
                }
            }
            Matcher::Regex(re) => re.is_match(haystack),
        }
    }

    fn find_range(&self, haystack: &str) -> Option<(usize, usize)> {
        match self {
            Matcher::Fixed(needle, ci) => {
                let pos = if *ci {
                    haystack.to_lowercase().find(&needle.to_lowercase())
                } else {
                    haystack.find(needle.as_str())
                }?;
                Some((pos, pos + needle.len()))
            }
            Matcher::Regex(re) => re.find(haystack).map(|m| (m.start(), m.end())),
        }
    }
}

#[allow(clippy::fn_params_excessive_bools)]
fn build_matchers(
    patterns: &[String],
    extended: bool,
    fixed: bool,
    ignore_case: bool,
    whole_word: bool,
    whole_line: bool,
) -> Result<Vec<Matcher>, ExecError> {
    patterns
        .iter()
        .map(|pat| {
            if fixed {
                return Ok(Matcher::Fixed(pat.clone(), ignore_case));
            }
            let mut p = pat.clone();
            if whole_word {
                p = format!(r"\b{}\b", regex::escape(&p));
            } else if whole_line {
                p = format!(r"^{p}$");
            }
            let mut builder = regex::RegexBuilder::new(&p);
            if !extended {
                // BRE: escape special chars not in BRE
                // For simplicity, use regex crate as ERE and hope patterns are compatible
            }
            builder.case_insensitive(ignore_case);
            builder
                .build()
                .map(Matcher::Regex)
                .map_err(|e| ExecError::InvalidArgument(e.to_string()))
        })
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::needless_range_loop
)]
fn grep_reader(
    reader: &mut dyn BufRead,
    fname: Option<&str>,
    matchers: &[Matcher],
    invert: bool,
    count_only: bool,
    files_with: bool,
    files_without: bool,
    line_num: bool,
    show_filename: bool,
    only_match: bool,
    quiet: bool,
    max_count: Option<usize>,
    before_ctx: usize,
    after_ctx: usize,
    null_term: bool,
) -> Result<bool, ExecError> {
    let record_sep = if null_term { b'\0' } else { b'\n' };
    let mut lines: Vec<String> = Vec::new();
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

    let matched_indices: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            let any = matchers.iter().any(|m| m.is_match(line));
            if invert { !any } else { any }
        })
        .map(|(i, _)| i)
        .collect();

    let any_match = !matched_indices.is_empty();

    if count_only {
        let n = max_count.map_or(matched_indices.len(), |m| matched_indices.len().min(m));
        if show_filename {
            println!("{}:{n}", fname.unwrap_or(""));
        } else {
            println!("{n}");
        }
        return Ok(any_match);
    }

    if files_with {
        if any_match {
            println!("{}", fname.unwrap_or(""));
        }
        return Ok(any_match);
    }
    if files_without {
        if !any_match {
            println!("{}", fname.unwrap_or(""));
        }
        return Ok(any_match);
    }
    if quiet {
        return Ok(any_match);
    }

    let mut printed = 0usize;
    let mut printed_indices = std::collections::HashSet::new();

    for &idx in &matched_indices {
        if max_count.is_some_and(|m| printed >= m) {
            break;
        }

        // Context lines before
        let start = idx.saturating_sub(before_ctx);
        for ci in start..idx {
            if printed_indices.insert(ci) {
                print_line(
                    &lines[ci],
                    fname,
                    ci + 1,
                    line_num,
                    show_filename,
                    false,
                    only_match,
                    matchers,
                );
            }
        }
        // Match line
        if printed_indices.insert(idx) {
            print_line(
                &lines[idx],
                fname,
                idx + 1,
                line_num,
                show_filename,
                true,
                only_match,
                matchers,
            );
            printed += 1;
        }
        // Context lines after
        let end = (idx + after_ctx + 1).min(lines.len());
        for ci in (idx + 1)..end {
            if printed_indices.insert(ci) {
                print_line(
                    &lines[ci],
                    fname,
                    ci + 1,
                    line_num,
                    show_filename,
                    false,
                    only_match,
                    matchers,
                );
            }
        }
    }

    Ok(any_match)
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn print_line(
    line: &str,
    fname: Option<&str>,
    lineno: usize,
    show_lineno: bool,
    show_fname: bool,
    is_match: bool,
    only_match: bool,
    matchers: &[Matcher],
) {
    let prefix = match (show_fname, fname) {
        (true, Some(f)) => format!("{f}:"),
        _ => String::new(),
    };
    let linenum_part = if show_lineno {
        format!("{lineno}:")
    } else {
        String::new()
    };

    if only_match && is_match {
        for m in matchers {
            if let Some((start, end)) = m.find_range(line) {
                println!("{prefix}{linenum_part}{}", &line[start..end]);
            }
        }
    } else {
        println!("{prefix}{linenum_part}{line}");
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>, include: Option<&str>, exclude: Option<&str>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(exc) = exclude {
                if utils::glob_match(exc, &name) {
                    continue;
                }
            }
            if path.is_dir() {
                collect_files(&path, out, include, exclude);
            } else {
                if let Some(inc) = include {
                    if !utils::glob_match(inc, &name) {
                        continue;
                    }
                }
                out.push(path);
            }
        }
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
    fn test_grep_match() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello world\nfoo bar\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Grep.run(&["hello".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_grep_no_match() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello world\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Grep.run(&["zzz".into(), "f.txt".into()], &mut ctx).unwrap(),
            1
        );
    }

    #[test]
    fn test_grep_case_insensitive() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"Hello World\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Grep.run(&["-i".into(), "hello".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_grep_invert() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"foo\nbar\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Grep.run(&["-v".into(), "foo".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_grep_count() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"a\na\nb\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Grep.run(&["-c".into(), "a".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_grep_missing_pattern_errors() {
        let mut ctx = ShellContext::new();
        assert!(Grep.run(&[], &mut ctx).is_err());
    }
}

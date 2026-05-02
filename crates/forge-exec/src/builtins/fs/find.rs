use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Find;

impl BuiltinCommand for Find {
    fn name(&self) -> &'static str {
        "find"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let follow_all = utils::has_flag(args, "-L");
        let follow_cmd = utils::has_flag(args, "-H");
        let depth_first = utils::has_flag(args, "-d");
        let _no_xdev = utils::has_flag(args, "-x");

        let (paths, expr_args) = split_paths_and_expr(args);
        let roots: Vec<PathBuf> = if paths.is_empty() {
            vec![ctx.cwd.clone()]
        } else {
            paths
                .iter()
                .map(|p| {
                    let path = Path::new(p);
                    if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        ctx.cwd.join(path)
                    }
                })
                .collect()
        };

        let expr = parse_expr(&expr_args);

        for root in &roots {
            let follow = follow_all || follow_cmd;
            walk(root, root, 0, follow, depth_first, &expr)?;
        }
        Ok(0)
    }
}

#[derive(Debug, Clone)]
enum Expr {
    True,
    Name(String, bool), // pattern, case_insensitive
    Path(String),
    Regex(String),
    Type(char),
    MaxDepth(usize),
    MinDepth(usize),
    Newer(PathBuf),
    Mtime(TimeComp),
    Mmin(TimeComp),
    Atime(TimeComp),
    Amin(TimeComp),
    Ctime(TimeComp),
    Cmin(TimeComp),
    Size(SizeComp),
    Empty,
    Perm(u32),
    Uid(u32),
    Gid(u32),
    Links(u64),
    Inum(u64),
    Print,
    Print0,
    Delete,
    Prune,
    Quit,
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
struct TimeComp {
    cmp: Cmp,
    n: i64,
}

#[derive(Debug, Clone)]
struct SizeComp {
    cmp: Cmp,
    n: u64,
    unit: char,
}

#[derive(Debug, Clone)]
enum Cmp {
    Exact,
    More,
    Less,
}

fn parse_time_arg(s: &str) -> TimeComp {
    if let Some(rest) = s.strip_prefix('+') {
        TimeComp {
            cmp: Cmp::More,
            n: rest.parse().unwrap_or(0),
        }
    } else if let Some(rest) = s.strip_prefix('-') {
        TimeComp {
            cmp: Cmp::Less,
            n: rest.parse().unwrap_or(0),
        }
    } else {
        TimeComp {
            cmp: Cmp::Exact,
            n: s.parse().unwrap_or(0),
        }
    }
}

fn parse_size_arg(s: &str) -> SizeComp {
    let (cmp, rest) = if let Some(r) = s.strip_prefix('+') {
        (Cmp::More, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (Cmp::Less, r)
    } else {
        (Cmp::Exact, s)
    };
    let (n_str, unit) = if rest.ends_with(|c: char| c.is_ascii_alphabetic()) {
        let u = rest.chars().last().unwrap();
        (&rest[..rest.len() - 1], u)
    } else {
        (rest, 'c')
    };
    SizeComp {
        cmp,
        n: n_str.parse().unwrap_or(0),
        unit,
    }
}

fn parse_expr(tokens: &[&str]) -> Expr {
    let (expr, _) = parse_or(tokens, 0);
    expr
}

fn parse_or(tokens: &[&str], pos: usize) -> (Expr, usize) {
    let (mut left, mut pos) = parse_and(tokens, pos);
    while pos < tokens.len() && (tokens[pos] == "-or" || tokens[pos] == "-o") {
        let (right, new_pos) = parse_and(tokens, pos + 1);
        left = Expr::Or(Box::new(left), Box::new(right));
        pos = new_pos;
    }
    (left, pos)
}

fn parse_and(tokens: &[&str], pos: usize) -> (Expr, usize) {
    let (mut left, mut pos) = parse_not(tokens, pos);
    while pos < tokens.len() && tokens[pos] != "-or" && tokens[pos] != "-o" && tokens[pos] != ")" {
        if tokens[pos] == "-and" || tokens[pos] == "-a" {
            pos += 1;
        }
        if pos >= tokens.len() || tokens[pos] == "-or" || tokens[pos] == "-o" || tokens[pos] == ")"
        {
            break;
        }
        let (right, new_pos) = parse_not(tokens, pos);
        left = Expr::And(Box::new(left), Box::new(right));
        pos = new_pos;
    }
    (left, pos)
}

fn parse_not(tokens: &[&str], pos: usize) -> (Expr, usize) {
    if pos < tokens.len() && (tokens[pos] == "!" || tokens[pos] == "-not") {
        let (inner, new_pos) = parse_primary(tokens, pos + 1);
        return (Expr::Not(Box::new(inner)), new_pos);
    }
    parse_primary(tokens, pos)
}

fn parse_primary(tokens: &[&str], pos: usize) -> (Expr, usize) {
    if pos >= tokens.len() {
        return (Expr::True, pos);
    }
    match tokens[pos] {
        "(" => {
            let (inner, new_pos) = parse_or(tokens, pos + 1);
            let skip = usize::from(new_pos < tokens.len() && tokens[new_pos] == ")");
            (inner, new_pos + skip)
        }
        "-name" if pos + 1 < tokens.len() => {
            (Expr::Name(tokens[pos + 1].to_string(), false), pos + 2)
        }
        "-iname" if pos + 1 < tokens.len() => {
            (Expr::Name(tokens[pos + 1].to_string(), true), pos + 2)
        }
        "-path" if pos + 1 < tokens.len() => (Expr::Path(tokens[pos + 1].to_string()), pos + 2),
        "-regex" if pos + 1 < tokens.len() => (Expr::Regex(tokens[pos + 1].to_string()), pos + 2),
        "-type" if pos + 1 < tokens.len() => (
            Expr::Type(tokens[pos + 1].chars().next().unwrap_or('f')),
            pos + 2,
        ),
        "-maxdepth" if pos + 1 < tokens.len() => (
            Expr::MaxDepth(tokens[pos + 1].parse().unwrap_or(usize::MAX)),
            pos + 2,
        ),
        "-mindepth" if pos + 1 < tokens.len() => (
            Expr::MinDepth(tokens[pos + 1].parse().unwrap_or(0)),
            pos + 2,
        ),
        "-newer" if pos + 1 < tokens.len() => {
            (Expr::Newer(PathBuf::from(tokens[pos + 1])), pos + 2)
        }
        "-mtime" if pos + 1 < tokens.len() => {
            (Expr::Mtime(parse_time_arg(tokens[pos + 1])), pos + 2)
        }
        "-mmin" if pos + 1 < tokens.len() => (Expr::Mmin(parse_time_arg(tokens[pos + 1])), pos + 2),
        "-atime" if pos + 1 < tokens.len() => {
            (Expr::Atime(parse_time_arg(tokens[pos + 1])), pos + 2)
        }
        "-amin" if pos + 1 < tokens.len() => (Expr::Amin(parse_time_arg(tokens[pos + 1])), pos + 2),
        "-ctime" if pos + 1 < tokens.len() => {
            (Expr::Ctime(parse_time_arg(tokens[pos + 1])), pos + 2)
        }
        "-cmin" if pos + 1 < tokens.len() => (Expr::Cmin(parse_time_arg(tokens[pos + 1])), pos + 2),
        "-size" if pos + 1 < tokens.len() => (Expr::Size(parse_size_arg(tokens[pos + 1])), pos + 2),
        "-empty" => (Expr::Empty, pos + 1),
        "-perm" if pos + 1 < tokens.len() => (
            Expr::Perm(
                u32::from_str_radix(tokens[pos + 1].trim_start_matches('0'), 8).unwrap_or(0),
            ),
            pos + 2,
        ),
        "-uid" if pos + 1 < tokens.len() => {
            (Expr::Uid(tokens[pos + 1].parse().unwrap_or(0)), pos + 2)
        }
        "-gid" if pos + 1 < tokens.len() => {
            (Expr::Gid(tokens[pos + 1].parse().unwrap_or(0)), pos + 2)
        }
        "-links" if pos + 1 < tokens.len() => {
            (Expr::Links(tokens[pos + 1].parse().unwrap_or(0)), pos + 2)
        }
        "-inum" if pos + 1 < tokens.len() => {
            (Expr::Inum(tokens[pos + 1].parse().unwrap_or(0)), pos + 2)
        }
        "-print" => (Expr::Print, pos + 1),
        "-print0" => (Expr::Print0, pos + 1),
        "-delete" => (Expr::Delete, pos + 1),
        "-prune" => (Expr::Prune, pos + 1),
        "-quit" => (Expr::Quit, pos + 1),
        // skip unknown primaries with their argument
        _ => (Expr::True, pos + 1),
    }
}

fn walk(
    root: &Path,
    path: &Path,
    depth: usize,
    follow: bool,
    depth_first: bool,
    expr: &Expr,
) -> Result<bool, ExecError> {
    let meta = if follow {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    };
    let Ok(meta) = meta else {
        eprintln!("find: {}: No such file or directory", path.display());
        return Ok(true);
    };

    if !depth_first {
        match eval(expr, path, root, &meta, depth)? {
            EvalResult::Print => println!("{}", path.display()),
            EvalResult::Print0 => {
                print!("{}\0", path.display());
            }
            EvalResult::Delete => {
                delete_path(path, &meta)?;
                return Ok(true);
            }
            EvalResult::Quit => return Ok(false),
            EvalResult::Prune => return Ok(true),
            EvalResult::Skip => {}
        }
    }

    if meta.is_dir() {
        let mut entries: Vec<_> = match std::fs::read_dir(path) {
            Ok(rd) => rd.filter_map(std::result::Result::ok).collect(),
            Err(_) => vec![],
        };
        entries.sort_by_key(|e| e.file_name().to_string_lossy().to_lowercase());

        for entry in entries {
            let cont = walk(root, &entry.path(), depth + 1, follow, depth_first, expr)?;
            if !cont {
                return Ok(false);
            }
        }
    }

    if depth_first {
        match eval(expr, path, root, &meta, depth)? {
            EvalResult::Print => println!("{}", path.display()),
            EvalResult::Print0 => {
                print!("{}\0", path.display());
            }
            EvalResult::Delete => {
                delete_path(path, &meta)?;
            }
            EvalResult::Quit => return Ok(false),
            EvalResult::Prune | EvalResult::Skip => {}
        }
    }

    Ok(true)
}

enum EvalResult {
    Print,
    Print0,
    Delete,
    Quit,
    Prune,
    Skip,
}

fn eval(
    expr: &Expr,
    path: &Path,
    _root: &Path,
    meta: &std::fs::Metadata,
    depth: usize,
) -> Result<EvalResult, ExecError> {
    let matched = matches_expr(expr, path, meta, depth)?;
    if matched {
        // Determine the action from the expression
        if contains_action(expr, "print0") {
            return Ok(EvalResult::Print0);
        }
        if contains_action(expr, "delete") {
            return Ok(EvalResult::Delete);
        }
        if contains_action(expr, "quit") {
            return Ok(EvalResult::Quit);
        }
        if contains_action(expr, "prune") {
            return Ok(EvalResult::Prune);
        }
        Ok(EvalResult::Print)
    } else {
        Ok(EvalResult::Skip)
    }
}

fn contains_action(expr: &Expr, action: &str) -> bool {
    match expr {
        Expr::Print0 => action == "print0",
        Expr::Delete => action == "delete",
        Expr::Quit => action == "quit",
        Expr::Prune => action == "prune",
        Expr::And(a, b) | Expr::Or(a, b) => {
            contains_action(a, action) || contains_action(b, action)
        }
        Expr::Not(inner) => contains_action(inner, action),
        _ => false,
    }
}

#[allow(clippy::too_many_lines)]
fn matches_expr(
    expr: &Expr,
    path: &Path,
    meta: &std::fs::Metadata,
    depth: usize,
) -> Result<bool, ExecError> {
    Ok(match expr {
        Expr::True | Expr::Print | Expr::Print0 | Expr::Delete | Expr::Quit => true,
        Expr::Prune => false,

        Expr::Name(pattern, ci) => {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if *ci {
                utils::glob_match(&pattern.to_lowercase(), &name.to_lowercase())
            } else {
                utils::glob_match(pattern, &name)
            }
        }
        Expr::Path(pattern) => utils::glob_match(pattern, &path.to_string_lossy()),
        Expr::Regex(pattern) => {
            let re = regex::Regex::new(pattern)
                .map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
            re.is_match(&path.to_string_lossy())
        }
        Expr::Type(t) => match t {
            'f' => meta.is_file() && !meta.file_type().is_symlink(),
            'd' => meta.is_dir(),
            'l' => meta.file_type().is_symlink(),
            _ => false,
        },
        Expr::MaxDepth(n) => depth <= *n,
        Expr::MinDepth(n) => depth >= *n,
        Expr::Newer(ref_path) => {
            let ref_mtime = std::fs::metadata(ref_path)?.modified()?;
            meta.modified()? > ref_mtime
        }
        Expr::Mtime(tc) => {
            let age_days = age_in_units(meta.modified()?, 86400);
            cmp_time(age_days, tc)
        }
        Expr::Mmin(tc) => {
            let age_mins = age_in_units(meta.modified()?, 60);
            cmp_time(age_mins, tc)
        }
        Expr::Atime(tc) => {
            let age_days = age_in_units(meta.accessed()?, 86400);
            cmp_time(age_days, tc)
        }
        Expr::Amin(tc) => {
            let age_mins = age_in_units(meta.accessed()?, 60);
            cmp_time(age_mins, tc)
        }
        Expr::Ctime(tc) => {
            let st = get_ctime_secs(meta);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            let age_days = (now - st) / 86400;
            cmp_time(age_days, tc)
        }
        Expr::Cmin(tc) => {
            let st = get_ctime_secs(meta);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            let age_mins = (now - st) / 60;
            cmp_time(age_mins, tc)
        }
        Expr::Size(sc) => {
            let bytes = meta.len();
            let effective = match sc.unit {
                'k' => bytes.div_ceil(1024),
                'M' => bytes.div_ceil(1024 * 1024),
                'G' => bytes.div_ceil(1024 * 1024 * 1024),
                _ => bytes,
            };
            match sc.cmp {
                Cmp::Exact => effective == sc.n,
                Cmp::More => effective > sc.n,
                Cmp::Less => effective < sc.n,
            }
        }
        Expr::Empty => {
            if meta.is_dir() {
                std::fs::read_dir(path)?.next().is_none()
            } else {
                meta.len() == 0
            }
        }
        Expr::Perm(mode) => get_mode(meta) & 0o777 == *mode,
        Expr::Uid(uid) => get_uid(meta) == *uid,
        Expr::Gid(gid) => get_gid(meta) == *gid,
        Expr::Links(n) => get_nlinks(meta) == *n,
        Expr::Inum(n) => get_inode(meta) == *n,
        Expr::Not(inner) => !matches_expr(inner, path, meta, depth)?,
        Expr::And(a, b) => {
            matches_expr(a, path, meta, depth)? && matches_expr(b, path, meta, depth)?
        }
        Expr::Or(a, b) => {
            matches_expr(a, path, meta, depth)? || matches_expr(b, path, meta, depth)?
        }
    })
}

fn age_in_units(t: SystemTime, unit_secs: u64) -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let then = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    i64::try_from((now - then) / unit_secs).unwrap_or(i64::MAX)
}

fn cmp_time(age: i64, tc: &TimeComp) -> bool {
    match tc.cmp {
        Cmp::Exact => age == tc.n,
        Cmp::More => age > tc.n,
        Cmp::Less => age < tc.n,
    }
}

fn delete_path(path: &Path, meta: &std::fs::Metadata) -> Result<(), ExecError> {
    if meta.is_dir() {
        std::fs::remove_dir(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn split_paths_and_expr(args: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut paths = Vec::new();
    let mut i = 0;
    // Skip global flags
    while i < args.len() {
        match args[i].as_str() {
            "-H" | "-L" | "-P" | "-E" | "-d" | "-x" => {
                i += 1;
            }
            s if s.starts_with('-') || s == "!" || s == "(" => break,
            _ => {
                paths.push(args[i].as_str());
                i += 1;
            }
        }
    }
    let expr: Vec<&str> = args[i..].iter().map(std::string::String::as_str).collect();
    (paths, expr)
}

#[cfg(unix)]
fn get_mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.mode()
}
#[cfg(not(unix))]
fn get_mode(_meta: &std::fs::Metadata) -> u32 {
    0o644
}

#[cfg(unix)]
fn get_uid(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.uid()
}
#[cfg(not(unix))]
fn get_uid(_meta: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn get_gid(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    meta.gid()
}
#[cfg(not(unix))]
fn get_gid(_meta: &std::fs::Metadata) -> u32 {
    0
}

#[cfg(unix)]
fn get_nlinks(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.nlink()
}
#[cfg(not(unix))]
fn get_nlinks(_meta: &std::fs::Metadata) -> u64 {
    1
}

#[cfg(unix)]
fn get_inode(meta: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    meta.ino()
}
#[cfg(not(unix))]
fn get_inode(_meta: &std::fs::Metadata) -> u64 {
    0
}

#[cfg(unix)]
fn get_ctime_secs(meta: &std::fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    meta.ctime()
}
#[cfg(not(unix))]
fn get_ctime_secs(_meta: &std::fs::Metadata) -> i64 {
    0
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
    fn test_find_basic() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Find.run(&[], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_find_name() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), b"").unwrap();
        std::fs::write(tmp.path().join("b.rs"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Find.run(&["-name".into(), "*.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_find_type_d() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Find.run(&["-type".into(), "d".into()], &mut ctx).unwrap(),
            0
        );
    }

    #[test]
    fn test_find_maxdepth() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("deep.txt"), b"").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Find.run(&["-maxdepth".into(), "1".into()], &mut ctx)
                .unwrap(),
            0
        );
    }
}

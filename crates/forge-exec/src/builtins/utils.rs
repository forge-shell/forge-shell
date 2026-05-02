/// Returns true if `flag` is present in `args`.
/// Handles combined single-char flags: `-la` satisfies both `-l` and `-a`.
pub fn has_flag(args: &[String], flag: &str) -> bool {
    for arg in args {
        if arg == flag {
            return true;
        }
        if let Some(short) = flag.strip_prefix('-') {
            if short.len() == 1
                && arg.starts_with('-')
                && !arg.starts_with("--")
                && arg.len() > 1
                && arg[1..].contains(short)
            {
                return true;
            }
        }
    }
    false
}

/// Returns the value that immediately follows `flag` in `args`.
pub fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().map(String::as_str);
        }
    }
    None
}

/// Collect positional (non-flag) arguments.
/// `value_flags` names flags whose next token is their value (consumed, not positional).
pub fn positional_args<'a>(args: &'a [String], value_flags: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if value_flags.contains(&arg.as_str()) {
            i += 2; // skip flag + value
            continue;
        }
        if arg.starts_with('-') && arg.len() > 1 && arg != "--" {
            i += 1;
            continue;
        }
        if arg == "--" {
            out.extend(args[i + 1..].iter().map(String::as_str));
            break;
        }
        out.push(arg.as_str());
        i += 1;
    }
    out
}

/// Format bytes as human-readable using 1024 base (K/M/G/T).
pub fn format_size_human(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    if bytes < 1024 {
        return format!("{bytes}B");
    }
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1024.0 && unit_idx + 1 < UNITS.len() {
        value /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1}{}", value, UNITS[unit_idx])
}

/// Format bytes as human-readable using 1000 base (SI: KB/MB/GB).
pub fn format_size_si(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return format!("{bytes}B");
    }
    #[allow(clippy::cast_precision_loss)]
    let mut value = bytes as f64;
    let mut unit_idx = 0usize;
    while value >= 1000.0 && unit_idx + 1 < UNITS.len() {
        value /= 1000.0;
        unit_idx += 1;
    }
    format!("{:.1}{}", value, UNITS[unit_idx])
}

/// Format a `SystemTime` as `YYYY-MM-DD HH:MM:SS` (UTC).
pub fn format_time(st: std::time::SystemTime) -> String {
    #[allow(clippy::cast_possible_wrap)]
    let secs = match st.duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(_) => 0,
    };
    let (y, mo, d, h, mi, s) = secs_to_civil(secs);
    format!("{y}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}")
}

/// Convert a Unix timestamp (seconds) to (year, month, day, hour, min, sec).
fn secs_to_civil(secs: i64) -> (i64, i64, i64, i64, i64, i64) {
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y0 = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y0 + 1 } else { y0 };
    let rem = secs.rem_euclid(86_400);
    (y, m, d, rem / 3_600, (rem % 3_600) / 60, rem % 60)
}

/// Format Unix mode bits as a 10-char permission string (e.g. `drwxr-xr-x`).
#[cfg(unix)]
pub fn format_mode(mode: u32, is_dir: bool, is_link: bool) -> String {
    let ft = if is_link {
        'l'
    } else if is_dir {
        'd'
    } else {
        '-'
    };
    let bits = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    let mut s = String::with_capacity(10);
    s.push(ft);
    for (bit, ch) in bits {
        s.push(if mode & bit != 0 { ch } else { '-' });
    }
    s
}

#[cfg(not(unix))]
pub fn format_mode(_mode: u32, is_dir: bool, is_link: bool) -> String {
    if is_link {
        "lrwxrwxrwx".to_string()
    } else if is_dir {
        "drwxrwxrwx".to_string()
    } else {
        "-rw-r--r--".to_string()
    }
}

/// Simple glob match: supports `*` (any sequence) and `?` (any single char).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    glob_inner(pattern.as_bytes(), name.as_bytes())
}

fn glob_inner(pat: &[u8], s: &[u8]) -> bool {
    match pat.first() {
        None => s.is_empty(),
        Some(&b'*') => glob_inner(&pat[1..], s) || (!s.is_empty() && glob_inner(pat, &s[1..])),
        Some(&b'?') => !s.is_empty() && glob_inner(&pat[1..], &s[1..]),
        Some(p) => matches!(s.first(), Some(c) if c == p) && glob_inner(&pat[1..], &s[1..]),
    }
}

/// Parse an optional `-n N` / `--lines=N` value from args, returning the default if absent.
#[allow(dead_code)]
pub fn parse_count_flag(
    args: &[String],
    short: &str,
    long_prefix: &str,
    default: usize,
) -> Result<usize, String> {
    // --lines=N form
    for arg in args {
        if let Some(val) = arg.strip_prefix(long_prefix) {
            return val
                .parse::<usize>()
                .map_err(|_| format!("invalid count: {val}"));
        }
    }
    // -n N form
    if let Some(val) = flag_value(args, short) {
        // handle leading '+' or '-' (for tail/head)
        let trimmed = val.trim_start_matches('+');
        return trimmed
            .parse::<usize>()
            .map_err(|_| format!("invalid count: {val}"));
    }
    Ok(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_flag_exact() {
        let args: Vec<String> = vec!["--all".into(), "-l".into()];
        assert!(has_flag(&args, "--all"));
        assert!(has_flag(&args, "-l"));
        assert!(!has_flag(&args, "-a"));
    }

    #[test]
    fn test_has_flag_combined() {
        let args: Vec<String> = vec!["-la".into()];
        assert!(has_flag(&args, "-l"));
        assert!(has_flag(&args, "-a"));
        assert!(!has_flag(&args, "-r"));
    }

    #[test]
    fn test_format_size_human() {
        assert_eq!(format_size_human(0), "0B");
        assert_eq!(format_size_human(512), "512B");
        assert_eq!(format_size_human(1024), "1.0K");
        assert_eq!(format_size_human(1024 * 1024), "1.0M");
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(!glob_match("*.rs", "main.go"));
        assert!(glob_match("foo?", "food"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("he?lo", "hello"));
    }

    #[test]
    fn test_format_time_epoch() {
        assert_eq!(format_time(std::time::UNIX_EPOCH), "1970-01-01 00:00:00");
    }
}

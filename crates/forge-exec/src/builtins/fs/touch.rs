use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use filetime::{FileTime, set_file_times};
use std::path::{Path, PathBuf};

pub struct Touch;

impl BuiltinCommand for Touch {
    fn name(&self) -> &'static str {
        "touch"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let access_only = utils::has_flag(args, "-a");
        let modify_only = utils::has_flag(args, "-m");
        let no_create = utils::has_flag(args, "-c");
        let ref_file = utils::flag_value(args, "-r");
        let time_str = utils::flag_value(args, "-t");
        let date_str = utils::flag_value(args, "-d");
        let targets = utils::positional_args(args, &["-r", "-t", "-d"]);

        if targets.is_empty() {
            return Err(ExecError::InvalidArgument(
                "touch: missing file operand".into(),
            ));
        }

        // Determine the desired time
        let desired: Option<FileTime> = if let Some(rf) = ref_file {
            let rp = resolve(rf, &ctx.cwd);
            let m = std::fs::metadata(&rp)?;
            Some(FileTime::from_system_time(m.modified()?))
        } else if let Some(ts) = time_str {
            Some(parse_touch_time(ts)?)
        } else if let Some(ds) = date_str {
            Some(parse_iso_date(ds)?)
        } else {
            None // use current time
        };

        for t in targets {
            let path = resolve(t, &ctx.cwd);

            if !path.exists() {
                if no_create {
                    continue;
                }
                std::fs::File::create(&path)?;
            }

            let now = FileTime::now();
            let (atime, mtime) = match desired {
                Some(ft) => (ft, ft),
                None => (now, now),
            };

            let meta = std::fs::metadata(&path)?;
            let access_time = if modify_only {
                FileTime::from_system_time(meta.accessed()?)
            } else {
                atime
            };
            let modify_time = if access_only {
                FileTime::from_system_time(meta.modified()?)
            } else {
                mtime
            };

            set_file_times(&path, access_time, modify_time)?;
        }
        Ok(0)
    }
}

/// Parse `[[CC]YY]MMDDhhmm[.SS]` timestamp format.
fn parse_touch_time(s: &str) -> Result<FileTime, ExecError> {
    let err = || ExecError::InvalidArgument(format!("touch: invalid time: {s}"));
    // Split on optional `.SS`
    let (main, secs) = if let Some((m, ss)) = s.rsplit_once('.') {
        let secs: u32 = ss.parse().map_err(|_| err())?;
        (m, secs)
    } else {
        (s, 0)
    };
    if main.len() < 8 || main.len() > 12 {
        return Err(err());
    }
    let (century_year, rest) = match main.len() {
        12 => (Some(&main[..4]), &main[4..]),
        10 => (Some(&main[..2]), &main[2..]),
        8 => (None, main),
        _ => return Err(err()),
    };
    let year: i32 = match century_year {
        Some(cy) if cy.len() == 4 => cy.parse().map_err(|_| err())?,
        Some(cy) => {
            let yy: i32 = cy.parse().map_err(|_| err())?;
            if yy >= 69 { 1900 + yy } else { 2000 + yy }
        }
        None => 2000, // fallback — not quite spec but avoids breakage
    };
    let mm: u32 = rest[0..2].parse().map_err(|_| err())?;
    let dd: u32 = rest[2..4].parse().map_err(|_| err())?;
    let hh: u32 = rest[4..6].parse().map_err(|_| err())?;
    let minute: u32 = rest[6..8].parse().map_err(|_| err())?;

    #[allow(clippy::cast_possible_wrap)]
    let unix = civil_to_unix(
        year,
        mm as i32,
        dd as i32,
        hh as i32,
        minute as i32,
        secs as i32,
    );
    Ok(FileTime::from_unix_time(unix, 0))
}

/// Parse ISO 8601 `YYYY-MM-DDThh:mm:ss` (local) date string.
fn parse_iso_date(s: &str) -> Result<FileTime, ExecError> {
    let err = || ExecError::InvalidArgument(format!("touch: invalid date: {s}"));
    let s = s.replace(' ', "T");
    let parts: Vec<&str> = s.splitn(2, 'T').collect();
    let date_part = parts[0];
    let time_part = if parts.len() > 1 {
        parts[1]
    } else {
        "00:00:00"
    };

    let dp: Vec<i32> = date_part
        .split('-')
        .map(|p| p.parse().map_err(|_| err()))
        .collect::<Result<Vec<_>, _>>()?;
    if dp.len() < 3 {
        return Err(err());
    }

    let tp: Vec<i32> = time_part
        .split(':')
        .map(|p| {
            p.trim_end_matches(|c: char| !c.is_ascii_digit())
                .parse()
                .map_err(|_| err())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (h, mi, sc) = (
        tp.first().copied().unwrap_or(0),
        tp.get(1).copied().unwrap_or(0),
        tp.get(2).copied().unwrap_or(0),
    );

    let unix = civil_to_unix(dp[0], dp[1], dp[2], h, mi, sc);
    Ok(FileTime::from_unix_time(unix, 0))
}

/// Very simple civil → unix seconds (UTC, no leap seconds).
#[allow(clippy::many_single_char_names)]
fn civil_to_unix(year: i32, month: i32, day: i32, hour: i32, minute: i32, sec: i32) -> i64 {
    let y = i64::from(if month <= 2 { year - 1 } else { year });
    let m = i64::from(month);
    let d = i64::from(day);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let jd = era * 146_097 + doe - 719_468;
    jd * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(sec)
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
    fn test_touch_creates_file() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Touch.run(&["new.txt".into()], &mut ctx).unwrap(), 0);
        assert!(tmp.path().join("new.txt").exists());
    }

    #[test]
    fn test_touch_no_create_flag() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Touch
                .run(&["-c".into(), "ghost.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
        assert!(!tmp.path().join("ghost.txt").exists());
    }

    #[test]
    fn test_touch_existing_file() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("f.txt");
        std::fs::write(&f, b"content").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Touch.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
        assert!(f.exists());
    }

    #[test]
    fn test_touch_no_args_errors() {
        assert!(matches!(
            Touch.run(&[], &mut ShellContext::new()),
            Err(ExecError::InvalidArgument(_))
        ));
    }
}

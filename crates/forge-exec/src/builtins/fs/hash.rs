use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Hash;

impl BuiltinCommand for Hash {
    fn name(&self) -> &'static str {
        "hash"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let algo = utils::flag_value(args, "-a").unwrap_or("sha256");
        let bsd_tag = utils::has_flag(args, "--tag");
        let check = utils::has_flag(args, "-c") || utils::has_flag(args, "--check");
        let quiet = utils::has_flag(args, "-q") || utils::has_flag(args, "--quiet");
        let status = utils::has_flag(args, "--status");
        let ignore_missing = utils::has_flag(args, "--ignore-missing");

        let targets = utils::positional_args(args, &["-a"]);

        if targets.is_empty() {
            // hash stdin
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf)?;
            let digest = compute_digest(algo, &buf)?;
            if bsd_tag {
                println!("{} (-) = {}", algo_label(algo), digest);
            } else {
                println!("{digest}  -");
            }
            return Ok(0);
        }

        if check {
            return verify_checksums(targets[0], algo, quiet, status, ignore_missing, &ctx.cwd);
        }

        for t in &targets {
            let path = resolve(t, &ctx.cwd);
            let mut buf = Vec::new();
            std::fs::File::open(&path)?.read_to_end(&mut buf)?;
            let digest = compute_digest(algo, &buf)?;
            if bsd_tag {
                println!("{} ({}) = {}", algo_label(algo), path.display(), digest);
            } else {
                println!("{digest}  {}", path.display());
            }
        }
        Ok(0)
    }
}

fn algo_label(algo: &str) -> &str {
    match algo {
        "md5" => "MD5",
        "sha1" => "SHA1",
        "sha224" => "SHA224",
        "sha256" => "SHA256",
        "sha384" => "SHA384",
        "sha512" => "SHA512",
        other => other,
    }
}

fn compute_digest(algo: &str, data: &[u8]) -> Result<String, ExecError> {
    use md5::Digest as _;
    match algo {
        "md5" => {
            let result = md5::Md5::digest(data);
            Ok(hex::encode(result))
        }
        "sha1" => {
            let result = sha1::Sha1::digest(data);
            Ok(hex::encode(result))
        }
        "sha224" => {
            use sha2::Digest as _;
            let result = sha2::Sha224::digest(data);
            Ok(hex::encode(result))
        }
        "sha256" | "" => {
            use sha2::Digest as _;
            let result = sha2::Sha256::digest(data);
            Ok(hex::encode(result))
        }
        "sha384" => {
            use sha2::Digest as _;
            let result = sha2::Sha384::digest(data);
            Ok(hex::encode(result))
        }
        "sha512" => {
            use sha2::Digest as _;
            let result = sha2::Sha512::digest(data);
            Ok(hex::encode(result))
        }
        other => Err(ExecError::InvalidArgument(format!(
            "hash: unknown algorithm: {other}"
        ))),
    }
}

#[allow(clippy::fn_params_excessive_bools)]
fn verify_checksums(
    file: &str,
    _algo: &str,
    quiet: bool,
    status: bool,
    ignore_missing: bool,
    cwd: &Path,
) -> Result<i32, ExecError> {
    let path = resolve(file, cwd);
    let content = std::fs::read_to_string(&path)?;
    let mut failures = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Try BSD format: ALGO (filename) = HASH
        let (hash, filename) = if let Some(eq_pos) = line.rfind(" = ") {
            let hash = &line[eq_pos + 3..];
            let name_part = &line[..eq_pos];
            let fname = if let Some(p) = name_part.rfind('(') {
                name_part[p + 1..].trim_end_matches(')')
            } else {
                name_part
            };
            (hash.to_string(), fname.to_string())
        } else {
            // GNU format: HASH  filename
            let parts: Vec<&str> = line.splitn(3, "  ").collect();
            if parts.len() < 2 {
                continue;
            }
            (parts[0].to_string(), parts[1].to_string())
        };

        let fpath = resolve(&filename, cwd);
        if !fpath.exists() {
            if ignore_missing {
                continue;
            }
            if !status {
                eprintln!("{filename}: No such file or directory");
            }
            failures += 1;
            continue;
        }

        let mut buf = Vec::new();
        std::fs::File::open(&fpath)?.read_to_end(&mut buf)?;
        // Detect algo from hash length
        let algo_guess = match hash.len() {
            32 => "md5",
            40 => "sha1",
            56 => "sha224",
            96 => "sha384",
            128 => "sha512",
            _ => "sha256", // covers 64 (sha256) and unknown lengths
        };
        let computed = compute_digest(algo_guess, &buf)?;
        if computed == hash {
            if !quiet && !status {
                println!("{filename}: OK");
            }
        } else {
            if !status {
                println!("{filename}: FAILED");
            }
            failures += 1;
        }
    }

    if failures > 0 && !status {
        eprintln!("WARNING: {failures} computed checksum(s) did NOT match");
    }
    Ok(i32::from(failures != 0))
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
    fn test_hash_sha256() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Hash.run(&["f.txt".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_hash_md5() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Hash.run(&["-a".into(), "md5".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_hash_bsd_tag() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hello").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Hash.run(&["--tag".into(), "f.txt".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_hash_unknown_algo_errors() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.txt"), b"hi").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert!(
            Hash.run(&["-a".into(), "blake3".into(), "f.txt".into()], &mut ctx)
                .is_err()
        );
    }
}

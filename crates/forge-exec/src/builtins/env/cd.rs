use crate::builtins::BuiltinCommand;
use crate::{ExecError, ShellContext};

pub struct Cd;

impl BuiltinCommand for Cd {
    fn name(&self) -> &'static str {
        "cd"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let target = match args.first() {
            Some(p) if p == "-" => ctx.env.get("OLDPWD").cloned().unwrap_or_default(),
            Some(p) if p.starts_with("~/") => {
                let home = ctx
                    .env
                    .get("HOME")
                    .or_else(|| ctx.env.get("USERPROFILE"))
                    .cloned()
                    .unwrap_or_default();
                format!("{}/{}", home, &p[2..])
            }
            Some(p) => p.clone(),
            None => ctx
                .env
                .get("HOME")
                .or_else(|| ctx.env.get("USERPROFILE"))
                .cloned()
                .unwrap_or_default(),
        };

        let new_path = if std::path::Path::new(&target).is_absolute() {
            std::path::PathBuf::from(&target)
        } else {
            ctx.cwd.join(&target)
        };

        let canonical = new_path
            .canonicalize()
            .map_err(|_| ExecError::CommandNotFound(format!("cd: {target}: no such directory")))?;

        ctx.env
            .insert("OLDPWD".to_string(), ctx.cwd.to_string_lossy().to_string());
        ctx.env
            .insert("PWD".to_string(), canonical.to_string_lossy().to_string());
        ctx.cwd = canonical;
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;
    use tempfile::TempDir;

    fn ctx() -> ShellContext {
        ShellContext::new()
    }

    #[test]
    fn test_cd_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx();
        Cd.run(&[tmp.path().to_string_lossy().to_string()], &mut ctx)
            .unwrap();
        assert_eq!(ctx.cwd, tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn test_cd_relative_path() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut ctx = ctx();
        ctx.cwd = tmp.path().to_path_buf();
        Cd.run(&["sub".to_string()], &mut ctx).unwrap();
        assert_eq!(ctx.cwd, sub.canonicalize().unwrap());
    }

    #[test]
    fn test_cd_sets_oldpwd() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut ctx = ctx();
        let original = tmp.path().to_string_lossy().to_string();
        ctx.cwd = tmp.path().to_path_buf();
        Cd.run(&["sub".to_string()], &mut ctx).unwrap();
        assert_eq!(ctx.env.get("OLDPWD").unwrap(), &original);
    }

    #[test]
    fn test_cd_nonexistent_errors() {
        let mut ctx = ctx();
        assert!(
            Cd.run(&["/does/not/exist/xyz".to_string()], &mut ctx)
                .is_err()
        );
    }

    #[test]
    fn test_cd_dash_goes_to_oldpwd() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut ctx = ctx();
        ctx.cwd = tmp.path().to_path_buf();
        ctx.env
            .insert("OLDPWD".to_string(), sub.to_string_lossy().to_string());
        Cd.run(&["-".to_string()], &mut ctx).unwrap();
        assert_eq!(ctx.cwd, sub.canonicalize().unwrap());
    }

    #[test]
    fn test_cd_no_args_goes_home() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ctx();
        ctx.env
            .insert("HOME".to_string(), tmp.path().to_string_lossy().to_string());
        assert!(Cd.run(&[], &mut ctx).is_ok());
    }
}

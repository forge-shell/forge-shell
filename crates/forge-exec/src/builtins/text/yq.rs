use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Yq;

impl BuiltinCommand for Yq {
    fn name(&self) -> &'static str {
        "yq"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let raw_output = utils::has_flag(args, "-r");
        let compact = utils::has_flag(args, "-c");
        let null_input = utils::has_flag(args, "-n");
        let exit_status = utils::has_flag(args, "-e");
        let filter_file = utils::flag_value(args, "-f");

        let targets = utils::positional_args(args, &["-f"]);

        let (filter_str, file_args): (&str, &[&str]) = if null_input {
            let f = targets.first().copied().unwrap_or(".");
            (f, &[])
        } else if targets.is_empty() {
            (".", &[])
        } else {
            (targets[0], &targets[1..])
        };

        let filter_str = if let Some(ff) = filter_file {
            std::fs::read_to_string(ff)?
        } else {
            filter_str.to_string()
        };

        let read_yaml = |path: &Path| -> Result<serde_json::Value, ExecError> {
            let content = std::fs::read_to_string(path)?;
            let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
                .map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
            yaml_to_json(&yaml_val)
        };

        let mut last_output: Option<serde_json::Value> = None;

        let process = |input: serde_json::Value| -> Result<Vec<serde_json::Value>, ExecError> {
            crate::builtins::filter::eval_filter(&filter_str, input, &[])
                .map_err(ExecError::InvalidArgument)
        };

        let print_value = |v: &serde_json::Value| -> Result<(), ExecError> {
            if raw_output {
                if let serde_json::Value::String(s) = v {
                    println!("{s}");
                    return Ok(());
                }
            }
            let yaml_val = json_to_yaml(v)?;
            if compact {
                let s = serde_yaml::to_string(&yaml_val)
                    .map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
                print!("{}", s.trim());
                println!();
            } else {
                let s = serde_yaml::to_string(&yaml_val)
                    .map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
                print!("{s}");
            }
            Ok(())
        };

        if null_input {
            let outputs = process(serde_json::Value::Null)?;
            for v in &outputs {
                print_value(v)?;
                last_output = Some(v.clone());
            }
        } else if file_args.is_empty() {
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)?;
            let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content)
                .map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
            let input = yaml_to_json(&yaml_val)?;
            let outputs = process(input)?;
            for v in &outputs {
                print_value(v)?;
                last_output = Some(v.clone());
            }
        } else {
            for f in file_args {
                let path = resolve(f, &ctx.cwd);
                let input = read_yaml(&path)?;
                let outputs = process(input)?;
                for v in &outputs {
                    print_value(v)?;
                    last_output = Some(v.clone());
                }
            }
        }

        if exit_status {
            match &last_output {
                None => return Ok(5),
                Some(serde_json::Value::Null | serde_json::Value::Bool(false)) => {
                    return Ok(1);
                }
                _ => {}
            }
        }
        Ok(0)
    }
}

fn yaml_to_json(v: &serde_yaml::Value) -> Result<serde_json::Value, ExecError> {
    let s = serde_json::to_string(v).map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
    serde_json::from_str(&s).map_err(|e| ExecError::InvalidArgument(e.to_string()))
}

fn json_to_yaml(v: &serde_json::Value) -> Result<serde_yaml::Value, ExecError> {
    let s = serde_json::to_string(v).map_err(|e| ExecError::InvalidArgument(e.to_string()))?;
    serde_yaml::from_str(&s).map_err(|e| ExecError::InvalidArgument(e.to_string()))
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
    fn test_yq_identity() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.yaml"), b"name: Alice\nage: 30\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Yq.run(&[".".into(), "f.yaml".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_yq_field() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.yaml"), b"name: Bob\n").unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Yq.run(&[".name".into(), "f.yaml".into()], &mut ctx)
                .unwrap(),
            0
        );
    }
}

use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub struct Jq;

impl BuiltinCommand for Jq {
    fn name(&self) -> &'static str {
        "jq"
    }

    #[allow(clippy::too_many_lines)]
    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let null_input = utils::has_flag(args, "-n") || utils::has_flag(args, "--null-input");
        let raw_input = utils::has_flag(args, "-R") || utils::has_flag(args, "--raw-input");
        let slurp = utils::has_flag(args, "-s") || utils::has_flag(args, "--slurp");
        let raw_output = utils::has_flag(args, "-r") || utils::has_flag(args, "--raw-output");
        let join_output = utils::has_flag(args, "-j") || utils::has_flag(args, "--join-output");
        let compact = utils::has_flag(args, "-c") || utils::has_flag(args, "--compact-output");
        let sort_keys = utils::has_flag(args, "-S") || utils::has_flag(args, "--sort-keys");
        let exit_status = utils::has_flag(args, "-e") || utils::has_flag(args, "--exit-status");
        let tab_indent = utils::has_flag(args, "--tab");
        let indent = utils::flag_value(args, "--indent")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(2);
        let filter_file =
            utils::flag_value(args, "-f").or_else(|| utils::flag_value(args, "--from-file"));

        let targets = utils::positional_args(
            args,
            &["-f", "--from-file", "--indent", "--arg", "--argjson"],
        );

        // First positional is filter, rest are files (unless -n/null-input)
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

        // Collect --arg bindings
        let mut bindings: Vec<(String, Value)> = Vec::new();
        for w in args.windows(3) {
            if w[0] == "--arg" {
                bindings.push((w[1].clone(), Value::String(w[2].clone())));
            } else if w[0] == "--argjson" {
                if let Ok(v) = serde_json::from_str(&w[2]) {
                    bindings.push((w[1].clone(), v));
                }
            }
        }

        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let mut last_output: Option<Value> = None;

        let process = |input: Value| -> Result<Vec<Value>, ExecError> {
            crate::builtins::filter::eval_filter(&filter_str, input, &bindings)
                .map_err(ExecError::InvalidArgument)
        };

        if null_input {
            let outputs = process(Value::Null)?;
            for v in &outputs {
                print_json_value(
                    v,
                    raw_output,
                    join_output,
                    compact,
                    sort_keys,
                    tab_indent,
                    indent,
                    &mut out,
                )?;
                last_output = Some(v.clone());
            }
        } else if raw_input {
            let mut all_lines = Vec::new();
            let read_ri =
                |reader: &mut dyn BufRead, lines: &mut Vec<String>| -> Result<(), ExecError> {
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line)? == 0 {
                            break;
                        }
                        lines.push(line.trim_end_matches(['\n', '\r']).to_string());
                    }
                    Ok(())
                };
            if file_args.is_empty() {
                let stdin = std::io::stdin();
                read_ri(&mut stdin.lock(), &mut all_lines)?;
            } else {
                for f in file_args {
                    let path = resolve(f, &ctx.cwd);
                    read_ri(
                        &mut std::io::BufReader::new(std::fs::File::open(&path)?),
                        &mut all_lines,
                    )?;
                }
            }
            if slurp {
                let arr =
                    Value::Array(all_lines.iter().map(|l| Value::String(l.clone())).collect());
                let outputs = process(arr)?;
                for v in &outputs {
                    print_json_value(
                        v,
                        raw_output,
                        join_output,
                        compact,
                        sort_keys,
                        tab_indent,
                        indent,
                        &mut out,
                    )?;
                    last_output = Some(v.clone());
                }
            } else {
                for line in &all_lines {
                    let outputs = process(Value::String(line.clone()))?;
                    for v in &outputs {
                        print_json_value(
                            v,
                            raw_output,
                            join_output,
                            compact,
                            sort_keys,
                            tab_indent,
                            indent,
                            &mut out,
                        )?;
                        last_output = Some(v.clone());
                    }
                }
            }
        } else {
            let read_all_json = |reader: &mut dyn BufRead| -> Result<Vec<Value>, ExecError> {
                let mut content = String::new();
                reader.read_to_string(&mut content)?;
                let mut values = Vec::new();
                let de = serde_json::Deserializer::from_str(&content).into_iter::<Value>();
                for v in de {
                    values.push(v.map_err(|e| ExecError::InvalidArgument(e.to_string()))?);
                }
                Ok(values)
            };

            let mut all_values: Vec<Value> = Vec::new();
            if file_args.is_empty() {
                let stdin = std::io::stdin();
                all_values.extend(read_all_json(&mut stdin.lock())?);
            } else {
                for f in file_args {
                    let path = resolve(f, &ctx.cwd);
                    let file = std::fs::File::open(&path)?;
                    all_values.extend(read_all_json(&mut std::io::BufReader::new(file))?);
                }
            }

            if slurp {
                let input = Value::Array(all_values);
                let outputs = process(input)?;
                for o in &outputs {
                    print_json_value(
                        o,
                        raw_output,
                        join_output,
                        compact,
                        sort_keys,
                        tab_indent,
                        indent,
                        &mut out,
                    )?;
                    last_output = Some(o.clone());
                }
            } else {
                for v in all_values {
                    let outputs = process(v)?;
                    for o in &outputs {
                        print_json_value(
                            o,
                            raw_output,
                            join_output,
                            compact,
                            sort_keys,
                            tab_indent,
                            indent,
                            &mut out,
                        )?;
                        last_output = Some(o.clone());
                    }
                }
            }
        }

        if exit_status {
            match &last_output {
                None => return Ok(5),
                Some(Value::Null | Value::Bool(false)) => return Ok(1),
                _ => {}
            }
        }
        Ok(0)
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn print_json_value(
    v: &Value,
    raw: bool,
    join: bool,
    compact: bool,
    sort_keys: bool,
    tab_indent: bool,
    indent: usize,
    out: &mut dyn Write,
) -> Result<(), ExecError> {
    if raw {
        if let Value::String(s) = v {
            if join {
                write!(out, "{s}")?;
            } else {
                writeln!(out, "{s}")?;
            }
            return Ok(());
        }
    }
    let s = if compact {
        let v2 = if sort_keys {
            sort_json_keys(v)
        } else {
            v.clone()
        };
        serde_json::to_string(&v2)?
    } else {
        let v2 = if sort_keys {
            sort_json_keys(v)
        } else {
            v.clone()
        };
        if tab_indent {
            let s = serde_json::to_string_pretty(&v2)?;
            s.replace("  ", "\t")
        } else if indent != 2 {
            // Custom indent
            format_with_indent(&v2, indent)
        } else {
            serde_json::to_string_pretty(&v2)?
        }
    };
    if join {
        write!(out, "{s}")?;
    } else {
        writeln!(out, "{s}")?;
    }
    Ok(())
}

fn sort_json_keys(v: &Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut sorted: serde_json::Map<String, Value> = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), sort_json_keys(&map[k]));
            }
            Value::Object(sorted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(sort_json_keys).collect()),
        other => other.clone(),
    }
}

fn format_with_indent(v: &Value, indent: usize) -> String {
    serde_json::to_string_pretty(v)
        .unwrap_or_default()
        .lines()
        .map(|l| {
            let depth = l.len() - l.trim_start().len();
            let new_depth = (depth / 2) * indent;
            format!("{}{}", " ".repeat(new_depth), l.trim_start())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl From<serde_json::Error> for ExecError {
    fn from(e: serde_json::Error) -> Self {
        ExecError::InvalidArgument(e.to_string())
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
    fn test_jq_identity() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.json"), br#"{"a":1}"#).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(Jq.run(&[".".into(), "f.json".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_jq_field() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.json"), br#"{"name":"Alice"}"#).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Jq.run(&[".name".into(), "f.json".into()], &mut ctx)
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_jq_null_input() {
        let mut ctx = ShellContext::new();
        assert_eq!(Jq.run(&["-n".into(), "null".into()], &mut ctx).unwrap(), 0);
    }

    #[test]
    fn test_jq_compact() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("f.json"), br#"{"a":1}"#).unwrap();
        let mut ctx = ctx_in(&tmp);
        assert_eq!(
            Jq.run(&["-c".into(), ".".into(), "f.json".into()], &mut ctx)
                .unwrap(),
            0
        );
    }
}

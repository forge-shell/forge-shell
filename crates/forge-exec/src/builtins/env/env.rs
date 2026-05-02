use crate::builtins::{BuiltinCommand, utils};
use crate::{ExecError, ShellContext};

pub struct Env;

impl BuiltinCommand for Env {
    fn name(&self) -> &'static str {
        "env"
    }

    fn run(&self, args: &[String], ctx: &mut ShellContext) -> Result<i32, ExecError> {
        let ignore = utils::has_flag(args, "-i");
        let null_sep = utils::has_flag(args, "-0");
        let chdir = utils::flag_value(args, "-C");

        // Collect NAME=VALUE assignments and optional utility name from positional args
        let positional = utils::positional_args(args, &["-u", "-C", "-P", "-S"]);

        // Collect -u NAME unsets
        let unsets: Vec<&str> = {
            let mut u = Vec::new();
            let mut i = 0;
            while i < args.len() {
                if args[i] == "-u" {
                    if let Some(name) = args.get(i + 1) {
                        u.push(name.as_str());
                        i += 2;
                        continue;
                    }
                }
                i += 1;
            }
            u
        };

        // Split positional into NAME=VALUE assignments vs utility args
        let assign_end = positional.iter().position(|a| !a.contains('='));
        let assignments = &positional[..assign_end.unwrap_or(positional.len())];
        // utility execution not implemented in this milestone (requires exec)

        let mut env: std::collections::HashMap<String, String> = if ignore {
            std::collections::HashMap::new()
        } else {
            ctx.env.clone()
        };

        for name in &unsets {
            env.remove(*name);
        }

        for kv in assignments {
            if let Some((k, v)) = kv.split_once('=') {
                env.insert(k.to_string(), v.to_string());
            }
        }

        if let Some(dir) = chdir {
            let _ = dir; // chdir before utility; utility not yet supported
        }

        // Print environment sorted
        let sep = if null_sep { '\0' } else { '\n' };
        let mut pairs: Vec<_> = env.iter().collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (k, v) in &pairs {
            print!("{k}={v}{sep}");
        }
        if !null_sep {
            // println! already added newline above; nothing extra needed
        }

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;

    fn ctx() -> ShellContext {
        ShellContext::new()
    }

    #[test]
    fn test_env_exits_zero() {
        assert_eq!(Env.run(&[], &mut ctx()).unwrap(), 0);
    }

    #[test]
    fn test_env_ignore() {
        // -i should not error
        assert_eq!(Env.run(&["-i".into()], &mut ctx()).unwrap(), 0);
    }
}

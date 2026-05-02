use crate::builtins::BuiltinCommand;

pub struct Echo;

impl BuiltinCommand for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }

    fn run(
        &self,
        args: &[String],
        _ctx: &mut crate::ShellContext,
    ) -> Result<i32, crate::ExecError> {
        let mut no_newline = false;
        let mut escape = false;
        let mut rest_start = 0;

        for (i, arg) in args.iter().enumerate() {
            match arg.as_str() {
                "-n" => {
                    no_newline = true;
                    rest_start = i + 1;
                }
                "-e" => {
                    escape = true;
                    rest_start = i + 1;
                }
                "-E" => {
                    escape = false;
                    rest_start = i + 1;
                }
                "-ne" | "-en" => {
                    no_newline = true;
                    escape = true;
                    rest_start = i + 1;
                }
                "-nE" | "-En" => {
                    no_newline = true;
                    escape = false;
                    rest_start = i + 1;
                }
                _ => break,
            }
        }

        let text = args[rest_start..].join(" ");
        let output = if escape { process_escapes(&text) } else { text };

        if no_newline {
            print!("{output}");
        } else {
            println!("{output}");
        }
        Ok(0)
    }
}

fn process_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') | None => out.push('\\'),
            Some('a') => out.push('\x07'),
            Some('b') => out.push('\x08'),
            Some('c') => return out, // stop producing output
            Some('e') => out.push('\x1B'),
            Some('f') => out.push('\x0C'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('v') => out.push('\x0B'),
            Some('0') => {
                // up to 3 octal digits
                let mut oct = String::new();
                for _ in 0..3 {
                    if chars.peek().is_some_and(|c| c.is_ascii_digit() && *c < '8') {
                        oct.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Ok(n) = u8::from_str_radix(&oct, 8) {
                    out.push(n as char);
                }
            }
            Some('x') => {
                // up to 2 hex digits
                let mut hex = String::new();
                for _ in 0..2 {
                    if chars.peek().is_some_and(char::is_ascii_hexdigit) {
                        hex.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if let Ok(n) = u8::from_str_radix(&hex, 16) {
                    out.push(n as char);
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShellContext;

    fn ctx() -> ShellContext {
        ShellContext::new()
    }

    #[test]
    fn test_echo_basic() {
        assert_eq!(Echo.run(&["hello".into()], &mut ctx()).unwrap(), 0);
    }

    #[test]
    fn test_echo_no_newline() {
        assert_eq!(
            Echo.run(&["-n".into(), "hi".into()], &mut ctx()).unwrap(),
            0
        );
    }

    #[test]
    fn test_echo_empty() {
        assert_eq!(Echo.run(&[], &mut ctx()).unwrap(), 0);
    }

    #[test]
    fn test_echo_escapes() {
        assert_eq!(
            Echo.run(&["-e".into(), "a\\tb".into()], &mut ctx())
                .unwrap(),
            0
        );
    }

    #[test]
    fn test_process_escapes_tab() {
        assert_eq!(process_escapes("a\\tb"), "a\tb");
    }

    #[test]
    fn test_process_escapes_newline() {
        assert_eq!(process_escapes("a\\nb"), "a\nb");
    }

    #[test]
    fn test_process_escapes_stop() {
        assert_eq!(process_escapes("ab\\ccd"), "ab");
    }

    #[test]
    fn test_process_escapes_octal() {
        assert_eq!(process_escapes("\\0101"), "A");
    }

    #[test]
    fn test_process_escapes_hex() {
        assert_eq!(process_escapes("\\x41"), "A");
    }
}

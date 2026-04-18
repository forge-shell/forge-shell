#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod error;
mod parser;

pub use error::ParseError;
pub use parser::Parser;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod directive_tests {
    use super::*;
    use forge_ast::*;
    use forge_lexer::Lexer;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenise().expect("lex failed");
        Parser::new(tokens).parse().expect("parse failed")
    }

    fn parse_err(src: &str) -> ParseError {
        let tokens = Lexer::new(src).tokenise().expect("lex failed");
        Parser::new(tokens)
            .parse()
            .expect_err("expected parse error")
    }

    #[test]
    fn test_unix_shebang_parsed() {
        let prog = parse("#!/usr/bin/env forge\n");
        assert_eq!(prog.directives.len(), 1);
        assert!(matches!(
            prog.directives[0].kind,
            DirectiveKind::UnixShebang(_)
        ));
    }

    #[test]
    fn test_strict_directive() {
        let prog = parse("#!forge:strict = true\n");
        assert_eq!(prog.directives[0].kind, DirectiveKind::Strict(true));
    }

    #[test]
    fn test_overflow_directive() {
        let prog = parse(r#"#!forge:overflow = "saturate""#);
        assert_eq!(
            prog.directives[0].kind,
            DirectiveKind::Overflow(OverflowMode::Saturate)
        );
    }

    #[test]
    fn test_platform_multiple() {
        let prog = parse(r#"#!forge:platform = "linux,macos""#);
        assert_eq!(
            prog.directives[0].kind,
            DirectiveKind::Platform(vec![Platform::Linux, Platform::MacOs])
        );
    }

    #[test]
    fn test_require_env_parsed() {
        let prog = parse(r#"#!forge:require-env = "DATABASE_URL,API_KEY""#);
        if let DirectiveKind::RequireEnv(vars) = &prog.directives[0].kind {
            assert_eq!(vars, &vec!["DATABASE_URL", "API_KEY"]);
        } else {
            panic!("expected RequireEnv");
        }
    }

    #[test]
    fn test_multiple_directives_then_code() {
        let prog = parse("#!/usr/bin/env forge\n#!forge:strict = true\nlet x = 1\n");
        assert_eq!(prog.directives.len(), 2);
        assert_eq!(prog.stmts.len(), 1);
    }

    #[test]
    fn test_directive_after_statement_is_error() {
        let err = parse_err("let x = 1\n#!forge:strict = true\n");
        println!("{err:?}");
        assert!(matches!(err, ParseError::DirectiveAfterStatement { .. }));
    }

    #[test]
    fn test_invalid_overflow_value() {
        let err = parse_err(r#"#!forge:overflow = "explode""#);
        assert!(matches!(err, ParseError::InvalidDirectiveValue { .. }));
    }

    #[test]
    fn test_abi_directive_gives_helpful_error() {
        let err = parse_err(r#"#!forge:abi = "1""#);
        if let ParseError::InvalidDirectiveValue { reason, .. } = err {
            assert!(reason.contains("forge-plugin.toml"));
        } else {
            panic!("expected InvalidDirectiveValue");
        }
    }

    #[test]
    fn test_unknown_directive_preserved() {
        let prog = parse(r#"#!forge:future-key = "value""#);
        assert!(matches!(
            prog.directives[0].kind,
            DirectiveKind::Unknown { .. }
        ));
    }

    #[test]
    fn test_jobs_auto() {
        let prog = parse(r#"#!forge:jobs = "auto""#);
        assert_eq!(prog.directives[0].kind, DirectiveKind::Jobs(JobLimit::Auto));
    }

    #[test]
    fn test_jobs_count() {
        let prog = parse(r#"#!forge:jobs = "4""#);
        assert_eq!(
            prog.directives[0].kind,
            DirectiveKind::Jobs(JobLimit::Count(4))
        );
    }
}

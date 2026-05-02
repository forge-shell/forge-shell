//! Fixture-based integration tests for forge-parser.
//!
//! Each `.fgs` file in `tests/fixtures/` is paired with a `.expected` file.
//! The test runner parses the `.fgs` file and compares the debug output of
//! the resulting `Program` (or error message) against the `.expected` file.
//!
//! # Updating expected output
//!
//! If you intentionally change the parser output, delete the relevant
//! `.expected` file and run:
//!
//! ```bash
//! UPDATE_FIXTURES=1 cargo test --test integration_test
//! ```
//!
//! The test runner will regenerate all missing `.expected` files.
//! Review the diff with `git diff` before committing.

use forge_lexer::Lexer;
use forge_parser::Parser;
use std::path::Path;

/// Parse a `.fgs` file and return the formatted debug output of the result.
/// For successful parses: `format!("{:#?}", program)`.
/// For error parses: `format!("ERROR: {error}")`.
fn parse_fixture(source: &str) -> String {
    let tokens = match Lexer::new(source).tokenise() {
        Ok(t) => t,
        Err(e) => return format!("LEX_ERROR: {e}"),
    };

    match Parser::new(tokens).parse() {
        Ok(program) => format!("{:#?}", program),
        Err(e) => format!("PARSE_ERROR: {e}"),
    }
}

/// Run a single fixture test.
///
/// - Reads `<name>.fgs` as input.
/// - Reads `<name>.expected` as expected output.
/// - If `UPDATE_FIXTURES=1` is set and `.expected` is missing, generates it.
/// - Panics with a diff if output doesn't match expected.
fn run_fixture(name: &str) {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let fgs_path = fixtures_dir.join(format!("{name}.fgs"));
    let expected_path = fixtures_dir.join(format!("{name}.expected"));

    let source = std::fs::read_to_string(&fgs_path)
        .unwrap_or_else(|_| panic!("fixture not found: {}", fgs_path.display()));

    let actual = parse_fixture(&source);

    if !expected_path.exists() {
        if std::env::var("UPDATE_FIXTURES").is_ok() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("failed to write expected: {e}"));
            println!("Generated: {}", expected_path.display());
            return;
        }
        panic!(
            "Missing expected file: {}\n\
             Run with UPDATE_FIXTURES=1 to generate it.\n\
             Actual output:\n{}",
            expected_path.display(),
            actual
        );
    }

    let expected = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("failed to read: {}", expected_path.display()));

    // Trim trailing whitespace from both sides for comparison
    let actual_trimmed = actual.trim();
    let expected_trimmed = expected.trim();

    if actual_trimmed != expected_trimmed {
        panic!(
            "Fixture mismatch: {name}\n\
             \n\
             --- expected\n\
             +++ actual\n\
             {}\n\
             \n\
             Run with UPDATE_FIXTURES=1 to regenerate if this change is intentional.",
            diff_lines(expected_trimmed, actual_trimmed)
        );
    }
}

/// Produce a simple line-by-line diff for readable failure output.
fn diff_lines(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let max = expected_lines.len().max(actual_lines.len());
    let mut output = Vec::new();

    for i in 0..max {
        let e = expected_lines.get(i).copied().unwrap_or("<missing>");
        let a = actual_lines.get(i).copied().unwrap_or("<missing>");
        if e != a {
            output.push(format!("  line {}: expected: {e:?}", i + 1));
            output.push(format!("  line {}:   actual: {a:?}", i + 1));
        }
    }

    if output.is_empty() {
        "  (no line differences found — whitespace issue?)".to_string()
    } else {
        output.join("\n")
    }
}

// --- Success fixtures ---
#[test]
fn fixture_01_hello_world() {
    run_fixture("01_hello_world");
}

#[test]
fn fixture_02_variables() {
    run_fixture("02_variables");
}

#[test]
fn fixture_03_arithmetic() {
    run_fixture("03_arithmetic");
}

#[test]
fn fixture_04_function_def() {
    run_fixture("04_function_def");
}

#[test]
fn fixture_05_function_call() {
    run_fixture("05_function_call");
}

#[test]
fn fixture_06_conditionals() {
    run_fixture("06_conditionals");
}

#[test]
fn fixture_07_while_loop() {
    run_fixture("07_while_loop");
}

#[test]
fn fixture_08_imports() {
    run_fixture("08_imports");
}

#[test]
fn fixture_09_directives() {
    run_fixture("09_directives");
}

#[test]
fn fixture_10_env_vars() {
    run_fixture("10_env_vars");
}

#[test]
fn fixture_11_pipes() {
    run_fixture("11_pipes");
}

#[test]
fn fixture_12_mutable_variables() {
    run_fixture("12_mutable_variables");
}

// --- error fixtures ---
#[test]
fn fixture_err_01_invalid_token() {
    run_fixture("err_01_invalid_token");
}

#[test]
fn fixture_err_02_directive_after_statement() {
    run_fixture("err_02_directive_after_statement");
}

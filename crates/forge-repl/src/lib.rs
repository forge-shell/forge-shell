#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use anyhow::Result;
use forge_backend::platform_backend;
use forge_exec::builtins::BuiltinRegistry;
use forge_exec::{Executor, ShellContext};
use forge_hir::AstLowerer;
use forge_lexer::Lexer;
use forge_parser::Parser;
use rustyline::DefaultEditor;

pub struct Repl {
    context: ShellContext,
    #[allow(dead_code)]
    builtins: BuiltinRegistry,
    history_path: std::path::PathBuf,
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    #[must_use]
    pub fn new() -> Self {
        let history_path = dirs_home().join(".forge").join("history");
        Self {
            context: ShellContext::new(),
            builtins: BuiltinRegistry::new(),
            history_path,
        }
    }

    /// # Errors
    /// Returns an error if readline initialisation fails or a pipeline step fails.
    pub fn run(&mut self) -> Result<()> {
        let mut rl = DefaultEditor::new()
            .map_err(|e| anyhow::anyhow!("failed to initialise readline: {e}"))?;

        let _ = rl.load_history(&self.history_path);

        println!(
            "Forge Shell v{} — type 'exit' to quit",
            env!("CARGO_PKG_VERSION")
        );

        loop {
            let prompt = self.make_prompt();

            match rl.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }

                    // Ignore history errors — not fatal
                    let _ = rl.add_history_entry(&line);

                    if line == "exit" || line == "quit" {
                        break;
                    }

                    if let Err(e) = self.eval(&line) {
                        eprintln!("forge: {e}");
                    }
                }
                Err(rustyline::error::ReadlineError::Interrupted) => {
                    // Ctrl+C — clear line, continue
                }
                Err(rustyline::error::ReadlineError::Eof) => {
                    // Ctrl+D — exit
                    break;
                }
                Err(e) => {
                    eprintln!("forge: input error: {e}");
                    break;
                }
            }
        }

        std::fs::create_dir_all(
            self.history_path
                .parent()
                .unwrap_or(std::path::Path::new(".")),
        )?;
        let _ = rl.save_history(&self.history_path);

        Ok(())
    }

    fn eval(&mut self, source: &str) -> Result<()> {
        // Eval pipeline
        let tokens = Lexer::new(source)
            .tokenise()
            .map_err(|e| anyhow::anyhow!("lexer error: {e}"))?;

        let ast = Parser::new(tokens)
            .parse()
            .map_err(|e| anyhow::anyhow!("parser error: {e}"))?;

        let directives = ast.directives.clone();

        let mut lowerer = AstLowerer::new();
        for name in self.context.vars.keys() {
            lowerer.declare_global(name);
        }
        let hir = lowerer
            .lower(ast)
            .map_err(|e| anyhow::anyhow!("lowering error: {e}"))?;

        let platform = platform_backend();
        let plan = platform
            .lower(&hir)
            .map_err(|e| anyhow::anyhow!("platform error: {e}"))?;

        let mut executor = Executor::new(self.context.clone());
        executor
            .enforce_directives(&directives)
            .map_err(|e| anyhow::anyhow!("directive error: {e}"))?;
        executor
            .run(&plan)
            .map_err(|e| anyhow::anyhow!("execution error: {e}"))?;

        self.context = executor.context;

        Ok(())
    }

    fn make_prompt(&self) -> String {
        let cwd = self.context.cwd.display().to_string();
        let home = self
            .context
            .env
            .get("HOME")
            .or_else(|| self.context.env.get("USERPROFILE"));

        let short_cwd = if let Some(home) = home {
            cwd.replace(home.as_str(), "~")
        } else {
            cwd
        };

        format!("forge [{short_cwd}] > ")
    }
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_or_else(|_| std::path::PathBuf::from("."), std::path::PathBuf::from)
}

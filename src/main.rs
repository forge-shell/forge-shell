#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use anyhow::Result;
use forge_repl::Repl;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Repl::new().run()
}

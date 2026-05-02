// crates/forge-exec/src/mod
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod context;
pub mod error;

pub use context::ShellContext;
pub use error::ExecError;

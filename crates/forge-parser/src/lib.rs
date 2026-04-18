#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod error;
mod parser;

pub use error::ParseError;
pub use parser::Parser;

#[cfg(test)]
mod tests;

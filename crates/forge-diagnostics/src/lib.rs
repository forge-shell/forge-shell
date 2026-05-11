#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod bag;
pub mod code;
pub mod convert;
pub mod diagnostic;
pub mod render;

pub use bag::DiagnosticBag;
pub use code::ErrorCode;
pub use diagnostic::{Diagnostic, Severity};
pub use render::DiagnosticRenderer;

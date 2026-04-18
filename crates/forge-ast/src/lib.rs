#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod directive;
pub mod expr;
pub mod literal;
pub mod op;
pub mod pattern;
pub mod program;
pub mod stmt;
pub mod ty;

pub use directive::{Directive, DirectiveKind, JobLimit, OverflowMode, Platform};
pub use expr::{Arg, Block, Expr, InterpolatedPart, MatchArm};
pub use forge_types::Span;
pub use literal::Literal;
pub use op::{BinaryOp, UnaryOp};
pub use pattern::Pattern;
pub use program::Program;
pub use stmt::{
    EnumDef, EnumVariant, FnDef, ImportPath, ImportStmt, Param, Stmt, StructDef, StructField,
};
pub use ty::Type;

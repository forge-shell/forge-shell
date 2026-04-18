use crate::literal::Literal;
use crate::op::{BinaryOp, UnaryOp};
use crate::pattern::Pattern;
use crate::stmt::Stmt;

/// A sequence of statements with an optional trailing expression.
///
/// The tail expression is the block's value when used in expression position,
/// e.g. `fn foo() -> str { let x = 1; "result" }`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Trailing expression whose value becomes the block's value.
    pub tail: Option<Box<Expr>>,
}

/// A segment of an interpolated string literal `"hello {name}"`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum InterpolatedPart {
    Literal(String),
    Expr(Box<Expr>),
}

/// An argument in a function or command call.
///
/// `ForgeScript` supports three call forms (RFC-001 §16):
/// positional, `--flag` style, and `name: value` named arguments.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Positional(Expr),
    /// `name: value`
    Named {
        name: String,
        value: Expr,
    },
    /// `--flag`  →  name = true
    Flag(String),
    /// `--no-flag`  →  name = false
    NoFlag(String),
}

/// A single arm in a `match` expression.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
}

/// An expression node.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Ident(String),
    /// An environment variable reference: `$NAME`.
    /// The inner string is the variable name without `$`.
    /// Resolved at runtime — not subject to static name resolution.
    EnvVar(String),
    BinaryOp {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Arg>,
    },
    /// `"hello {name}"` — a string containing interpolated expressions.
    Interpolated(Vec<InterpolatedPart>),
    If {
        cond: Box<Expr>,
        then_branch: Block,
        else_branch: Option<Box<Expr>>,
    },
    Block(Block),
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// `spawn { ... }` or `spawn(ctx) { ... }`
    Spawn {
        ctx: Option<Box<Expr>>,
        body: Block,
    },
    /// `join! { spawn { ... }, spawn { ... } }`
    Join(Vec<Expr>),
    /// The `?` error propagation operator.
    Try(Box<Expr>),
    /// `base[index]`
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// `base.field`
    Field {
        base: Box<Expr>,
        field: String,
    },
    /// `receiver.method(args…)`
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Arg>,
    },
    Return(Option<Box<Expr>>),
    Break(Option<Box<Expr>>),
    Continue,
}

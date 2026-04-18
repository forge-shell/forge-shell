use crate::directive::Directive;
use crate::stmt::Stmt;

/// The root node of a parsed `ForgeScript` file.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Directives declared before the first statement (`#!` lines).
    pub directives: Vec<Directive>,
    /// The statements that make up the program body.
    pub stmts: Vec<Stmt>,
}

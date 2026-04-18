use crate::expr::{Block, Expr};
use crate::ty::Type;

/// A function parameter declaration.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// A `fn` definition.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<Param>,
    pub ret_ty: Option<Type>,
    pub body: Block,
}

/// A field in a struct definition.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
}

/// A `struct` definition.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
}

/// A variant inside an `enum` definition.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum EnumVariant {
    /// `Unit` — no payload.
    Unit(String),
    /// `Tuple` — positional fields, e.g. `Failed(str)`.
    Tuple(String, Vec<Type>),
    /// `Struct` — named fields, e.g. `Pending { retries: int }`.
    Struct(String, Vec<StructField>),
}

/// An `enum` definition.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

/// The path portion of an import statement.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum ImportPath {
    /// `forge::fs` — absolute crate-rooted path.
    Absolute(Vec<String>),
    /// `./utils` — path relative to the current file.
    Relative(Vec<String>),
}

/// An `import` statement.
///
/// Covers all three forms from RFC-001 §13:
/// - `import forge::fs`
/// - `import ./utils::{ read_config, write_output }`
/// - `import ./utils as u`
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub struct ImportStmt {
    pub path: ImportPath,
    /// `as alias`
    pub alias: Option<String>,
    /// `::{ item1, item2 }` — empty means import the module itself.
    pub items: Vec<String>,
}

/// A statement node.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let [mut] name[: Type] = value`
    Let {
        name: String,
        mutable: bool,
        ty: Option<Type>,
        value: Expr,
    },
    /// `const NAME[: Type] = value`
    Const {
        name: String,
        ty: Option<Type>,
        value: Expr,
    },
    /// `target = value`
    Assign {
        target: Expr,
        value: Expr,
    },
    /// An expression used as a statement.
    ExprStmt(Expr),
    FnDef(FnDef),
    StructDef(StructDef),
    EnumDef(EnumDef),
    Import(ImportStmt),
    /// `for var in iter { body }`
    For {
        var: String,
        iter: Expr,
        body: Block,
    },
    /// `while cond { body }`
    While {
        cond: Expr,
        body: Block,
    },
    /// `loop { body }`
    Loop(Block),
    /// `return [value]`
    Return(Option<Expr>),
}

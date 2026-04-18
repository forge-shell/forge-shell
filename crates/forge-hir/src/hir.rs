use forge_types::Span;

/// The root HIR node — a fully lowered `ForgeScript` program.
#[derive(Debug, Clone)]
pub struct HirProgram {
    /// Top-level function definitions (lifted from statements).
    pub fns: Vec<HirFnDef>,
    /// Resolved imports in declaration order.
    pub imports: Vec<HirImport>,
    /// Top-level statements (excluding fn defs and imports).
    pub stmts: Vec<HirStmt>,
}

/// A function definition after lowering.
#[derive(Debug, Clone)]
pub struct HirFnDef {
    pub name: String,
    pub params: Vec<HirParam>,
    pub body: Vec<HirStmt>,
    pub span: Span,
}

/// A function parameter after lowering.
#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub span: Span,
}

/// A resolved import.
#[derive(Debug, Clone)]
pub struct HirImport {
    /// Normalised import path (for diagnostics and cycle detection).
    pub path: String,
    /// Optional alias (`import x as y` → alias is "y").
    pub alias: Option<String>,
    pub span: Span,
}

/// A statement in the HIR.
/// Simpler than AST statements — let and assign are unified into Bind.
#[derive(Debug, Clone)]
pub enum HirStmt {
    /// Variable binding — covers `let`, `const`, and reassignment.
    Bind {
        name: String,
        mutable: bool,
        value: HirExpr,
        span: Span,
    },
    /// Expression evaluated for side effects (e.g. a command invocation).
    Eval { expr: HirExpr, span: Span },
    /// Return a value from a function.
    Return { value: HirExpr, span: Span },
    /// Conditional execution.
    If {
        cond: HirExpr,
        then: Vec<HirStmt>,
        else_: Vec<HirStmt>,
        span: Span,
    },
    /// While loop.
    While {
        cond: HirExpr,
        body: Vec<HirStmt>,
        span: Span,
    },
}

/// An expression in the HIR.
#[derive(Debug, Clone)]
pub enum HirExpr {
    /// A literal value.
    Literal(HirLiteral),
    /// A reference to a declared variable.
    Var { name: String, span: Span },
    /// An environment variable reference (`$NAME`).
    /// Not subject to scope checking — resolved at runtime.
    EnvVar { name: String, span: Span },
    /// A binary operation.
    BinOp {
        op: HirBinOp,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
        span: Span,
    },
    /// A unary operation.
    UnaryOp {
        op: HirUnaryOp,
        operand: Box<HirExpr>,
        span: Span,
    },
    /// A function call.
    Call {
        callee: String,
        args: Vec<HirExpr>,
        span: Span,
    },
    /// A pipe: left | right.
    Pipe {
        left: Box<HirExpr>,
        right: Box<HirExpr>,
        span: Span,
    },
    /// An if expression (produces a value).
    If {
        cond: Box<HirExpr>,
        then: Vec<HirStmt>,
        else_: Vec<HirStmt>,
        span: Span,
    },
    /// Field access: expr.field
    FieldAccess {
        target: Box<HirExpr>,
        field: String,
        span: Span,
    },
    /// Index access: expr[index]
    Index {
        target: Box<HirExpr>,
        index: Box<HirExpr>,
        span: Span,
    },
}

/// A literal value in the HIR.
#[derive(Debug, Clone, PartialEq)]
pub enum HirLiteral {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Null,
}

/// Binary operators in the HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Concat,
}

/// Unary operators in the HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirUnaryOp {
    Neg,
    Not,
}

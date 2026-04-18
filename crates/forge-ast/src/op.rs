/// Binary operator kind.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Saturating arithmetic: +|  -|  *|
    AddSat,
    SubSat,
    MulSat,
    // Wrapping arithmetic: +%  -%  *%
    AddWrap,
    SubWrap,
    MulWrap,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Logical
    And,
    Or,
    // Path join  (the `/` operator on path values)
    PathJoin,
    // Shell pipe
    Pipe,
}

/// Unary operator kind.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
}

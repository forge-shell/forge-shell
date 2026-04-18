use crate::literal::Literal;

/// A pattern used in `match` arms and destructuring binds.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_`
    Wildcard,
    /// A binding name, e.g. `x`.
    Ident(String),
    /// A literal constant, e.g. `42` or `"hello"`.
    Literal(Literal),
    /// A tuple pattern `(a, b)`.
    Tuple(Vec<Pattern>),
    /// A struct pattern `Config { host, port }`.
    Struct {
        name: String,
        fields: Vec<(String, Pattern)>,
    },
    /// A tuple-struct or enum-tuple variant, e.g. `Some(x)` or `Failed(msg)`.
    TupleStruct {
        name: String,
        fields: Vec<Pattern>,
    },
    // Built-in Result / Option shorthands kept as named variants for
    // parser ergonomics — they desugar to TupleStruct during HIR lowering.
    Ok(Box<Pattern>),
    Err(Box<Pattern>),
    Some(Box<Pattern>),
    None,
}

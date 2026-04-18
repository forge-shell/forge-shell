/// A literal value as it appears in source.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    /// `p"..."` — statically-validated path literal.
    Path(String),
    /// `r"..."` — statically-validated regex literal.
    Regex(String),
    /// `u"..."` — statically-validated URL literal.
    Url(String),
}

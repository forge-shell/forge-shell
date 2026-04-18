/// A `ForgeScript` type annotation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    // Primitive types
    Int,
    Float,
    Str,
    Bool,
    Path,
    Regex,
    Url,
    // Composite types
    Result(Box<Type>, Box<Type>),
    Option(Box<Type>),
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    /// `Task<T>` returned by `spawn { }`.
    Task(Box<Type>),
    /// User-defined named type (struct or enum).
    Named(String),
    /// `()` — the unit type.
    Unit,
}

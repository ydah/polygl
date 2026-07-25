use polygl_span::Span;

use crate::Symbol;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeExpr {
    pub kind: TypeKind,
    pub span: Span,
}

impl TypeExpr {
    #[must_use]
    pub const fn new(kind: TypeKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeKind {
    Unit,
    Int,
    Float,
    Bool,
    Str,
    Array(Box<TypeExpr>),
    Map(Box<TypeExpr>),
    Option(Box<TypeExpr>),
    Struct(Symbol),
    Vector(u8),
    Matrix(u8),
    Opaque(OpaqueType),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpaqueType {
    Mesh,
    Node,
    Material,
    Texture,
}

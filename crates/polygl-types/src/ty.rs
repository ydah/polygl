use std::fmt;

use polygl_hir::{OpaqueType, Symbol, TypeExpr, TypeKind};
use polygl_span::Span;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Unit,
    Int,
    Float,
    Bool,
    Str,
    Array(Box<Self>),
    Map(Box<Self>),
    Option(Box<Self>),
    Struct(Symbol),
    Vector(u8),
    Matrix(u8),
    Opaque(OpaqueType),
}

impl Type {
    pub(crate) fn is_value_type(&self) -> bool {
        match self {
            Self::Unit => false,
            Self::Array(element) | Self::Map(element) | Self::Option(element) => {
                element.is_value_type()
            }
            Self::Int
            | Self::Float
            | Self::Bool
            | Self::Str
            | Self::Struct(_)
            | Self::Vector(_)
            | Self::Matrix(_)
            | Self::Opaque(_) => true,
        }
    }

    pub(crate) fn from_expr(expression: &TypeExpr) -> Self {
        match &expression.kind {
            TypeKind::Unit => Self::Unit,
            TypeKind::Int => Self::Int,
            TypeKind::Float => Self::Float,
            TypeKind::Bool => Self::Bool,
            TypeKind::Str => Self::Str,
            TypeKind::Array(element) => Self::Array(Box::new(Self::from_expr(element))),
            TypeKind::Map(value) => Self::Map(Box::new(Self::from_expr(value))),
            TypeKind::Option(value) => Self::Option(Box::new(Self::from_expr(value))),
            TypeKind::Struct(name) => Self::Struct(name.clone()),
            TypeKind::Vector(size) => Self::Vector(*size),
            TypeKind::Matrix(size) => Self::Matrix(*size),
            TypeKind::Opaque(kind) => Self::Opaque(*kind),
        }
    }

    pub(crate) fn to_expr(&self, span: Span) -> TypeExpr {
        let kind = match self {
            Self::Unit => TypeKind::Unit,
            Self::Int => TypeKind::Int,
            Self::Float => TypeKind::Float,
            Self::Bool => TypeKind::Bool,
            Self::Str => TypeKind::Str,
            Self::Array(element) => TypeKind::Array(Box::new(element.to_expr(span))),
            Self::Map(value) => TypeKind::Map(Box::new(value.to_expr(span))),
            Self::Option(value) => TypeKind::Option(Box::new(value.to_expr(span))),
            Self::Struct(name) => TypeKind::Struct(name.clone()),
            Self::Vector(size) => TypeKind::Vector(*size),
            Self::Matrix(size) => TypeKind::Matrix(*size),
            Self::Opaque(kind) => TypeKind::Opaque(*kind),
        };
        TypeExpr::new(kind, span)
    }

    pub(crate) fn mangle(&self) -> String {
        match self {
            Self::Unit => "unit".to_owned(),
            Self::Int => "int".to_owned(),
            Self::Float => "float".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::Str => "str".to_owned(),
            Self::Array(element) => format!("array_{}", element.mangle()),
            Self::Map(value) => format!("map_{}", value.mangle()),
            Self::Option(value) => format!("option_{}", value.mangle()),
            Self::Struct(name) => format!("struct_{name}"),
            Self::Vector(size) => format!("vec{size}"),
            Self::Matrix(size) => format!("mat{size}"),
            Self::Opaque(kind) => format!("{kind:?}").to_ascii_lowercase(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => formatter.write_str("void"),
            Self::Int => formatter.write_str("int"),
            Self::Float => formatter.write_str("float"),
            Self::Bool => formatter.write_str("bool"),
            Self::Str => formatter.write_str("str"),
            Self::Array(element) => write!(formatter, "{element}[]"),
            Self::Map(value) => write!(formatter, "Map<str, {value}>"),
            Self::Option(value) => write!(formatter, "Option<{value}>"),
            Self::Struct(name) => name.fmt(formatter),
            Self::Vector(size) => write!(formatter, "vec{size}"),
            Self::Matrix(size) => write!(formatter, "mat{size}"),
            Self::Opaque(kind) => write!(formatter, "{kind:?}"),
        }
    }
}

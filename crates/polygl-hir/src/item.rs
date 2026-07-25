use polygl_span::Span;

use crate::{Block, Expr, Symbol, TypeExpr};

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Const(ConstDef),
    Entry(EntryPoint),
}

impl Item {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Function(item) => item.span,
            Self::Struct(item) => item.span,
            Self::Const(item) => item.span,
            Self::Entry(item) => item.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: Symbol,
    pub params: Vec<Param>,
    pub return_type: Option<TypeExpr>,
    pub body: Block,
    pub span: Span,
    pub domain: DomainHint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Symbol,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDef {
    pub name: Symbol,
    pub fields: Vec<FieldDef>,
    pub methods: Vec<Function>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDef {
    pub name: Symbol,
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConstDef {
    pub name: Symbol,
    pub ty: Option<TypeExpr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntryPoint {
    pub kind: EntryPointKind,
    pub params: Vec<Param>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryPointKind {
    Setup,
    Frame,
    OnEvent,
    Vertex(Symbol),
    Fragment(Symbol),
}

impl EntryPointKind {
    #[must_use]
    pub fn canonical_name(&self) -> String {
        match self {
            Self::Setup => "setup".to_owned(),
            Self::Frame => "frame".to_owned(),
            Self::OnEvent => "on_event".to_owned(),
            Self::Vertex(name) => format!("vertex_{name}"),
            Self::Fragment(name) => format!("fragment_{name}"),
        }
    }

    #[must_use]
    pub const fn domain(&self) -> DomainHint {
        match self {
            Self::Setup | Self::Frame | Self::OnEvent => DomainHint::Host,
            Self::Vertex(_) | Self::Fragment(_) => DomainHint::Gpu,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DomainHint {
    Auto,
    Host,
    Gpu,
}

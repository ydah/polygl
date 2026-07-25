use polygl_span::Span;
use polygl_types::Type;

use crate::{Block, Expr};

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub functions: Vec<Function>,
    pub structs: Vec<StructDef>,
    pub constants: Vec<Constant>,
    pub entries: Vec<EntryPoint>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Parameter>,
    pub result: Type,
    pub body: Block,
    pub domain: Domain,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EntryPoint {
    pub kind: EntryKind,
    pub params: Vec<Parameter>,
    pub result: Type,
    pub body: Block,
    pub domain: Domain,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Field>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constant {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
    pub domain: Domain,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Domain {
    Host,
    Gpu,
    Shared,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Setup,
    Frame,
    OnEvent,
    Vertex(String),
    Fragment(String),
}

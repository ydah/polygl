use polygl_span::Span;

use crate::{BuiltinId, Symbol, TypeExpr};

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    /// Filled by `polygl-types`; adapters leave this unset.
    pub ty: Option<TypeExpr>,
    pub span: Span,
}

impl Expr {
    #[must_use]
    pub const fn new(kind: ExprKind, span: Span) -> Self {
        Self {
            kind,
            ty: None,
            span,
        }
    }

    #[must_use]
    pub fn with_type(mut self, ty: TypeExpr) -> Self {
        self.ty = Some(ty);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Var(Symbol),
    Uniform {
        name: Symbol,
        declared: TypeExpr,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Call {
        callee: Callee,
        args: Vec<Expr>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Field {
        base: Box<Expr>,
        field: Symbol,
    },
    ArrayLength(Box<Expr>),
    Array(Vec<Expr>),
    Map(Vec<MapEntry>),
    Struct {
        name: Symbol,
        fields: Vec<FieldInit>,
    },
    Vector {
        size: u8,
        args: Vec<Expr>,
    },
    NilCheck(Box<Expr>),
    FalsyCheck(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldInit {
    pub name: Symbol,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Int(i32),
    Float(f64),
    Bool(bool),
    Str(String),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    DivInt,
    DivFloat,
    RemFloor,
    RemTrunc,
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    And,
    Or,
    StrConcat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Callee {
    User(Symbol),
    Method(Symbol),
    Builtin(BuiltinId),
}

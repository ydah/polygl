use polygl_span::Span;

use crate::{Expr, Symbol, TypeExpr};

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    #[must_use]
    pub const fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StmtKind {
    Let {
        name: Symbol,
        ty: Option<TypeExpr>,
        init: Expr,
    },
    Assign {
        target: Place,
        value: Expr,
    },
    Expr(Expr),
    If {
        condition: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    While {
        condition: Expr,
        body: Block,
    },
    For {
        variable: Symbol,
        range: RangeExpr,
        body: Block,
    },
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub kind: PlaceKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaceKind {
    Var(Symbol),
    Index { base: Expr, index: Expr },
    Field { base: Expr, field: Symbol },
}

#[derive(Clone, Debug, PartialEq)]
pub struct RangeExpr {
    pub start: Expr,
    pub end: Expr,
    pub inclusive: bool,
    pub span: Span,
}

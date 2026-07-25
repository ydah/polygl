use polygl_span::Span;
use polygl_types::Type;

use crate::Expr;

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}

impl Statement {
    #[must_use]
    pub const fn new(kind: StatementKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum StatementKind {
    Let {
        name: String,
        ty: Type,
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
        variable: String,
        range: Range,
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
    Variable(String),
    Index { base: Expr, index: Expr },
    Field { base: Expr, field: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Range {
    pub start: Expr,
    pub end: Expr,
    pub inclusive: bool,
    pub span: Span,
}

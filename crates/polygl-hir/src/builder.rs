use polygl_span::Span;

use crate::{
    BinOp, Block, BuiltinId, Callee, EntryPoint, EntryPointKind, Expr, ExprKind, Item, Literal,
    Module, Stmt, StmtKind, Symbol, UnOp,
};

/// Convenience factory for hand-written HIR and adapter lowering tests.
#[derive(Clone, Copy, Debug)]
pub struct HirBuilder {
    span: Span,
}

impl HirBuilder {
    #[must_use]
    pub const fn new(span: Span) -> Self {
        Self { span }
    }

    #[must_use]
    pub const fn span(self) -> Span {
        self.span
    }

    #[must_use]
    pub fn module(self, items: Vec<Item>) -> Module {
        Module {
            items,
            span: self.span,
        }
    }

    #[must_use]
    pub fn block(self, statements: Vec<Stmt>) -> Block {
        Block {
            statements,
            span: self.span,
        }
    }

    #[must_use]
    pub fn entry(self, kind: EntryPointKind, body: Block) -> Item {
        Item::Entry(EntryPoint {
            kind,
            params: Vec::new(),
            body,
            span: self.span,
        })
    }

    #[must_use]
    pub fn int(self, value: i32) -> Expr {
        self.literal(Literal::Int(value))
    }

    #[must_use]
    pub fn float(self, value: f64) -> Expr {
        self.literal(Literal::Float(value))
    }

    #[must_use]
    pub fn bool(self, value: bool) -> Expr {
        self.literal(Literal::Bool(value))
    }

    #[must_use]
    pub fn string(self, value: impl Into<String>) -> Expr {
        self.literal(Literal::Str(value.into()))
    }

    #[must_use]
    pub fn none(self) -> Expr {
        self.literal(Literal::None)
    }

    #[must_use]
    pub fn literal(self, literal: Literal) -> Expr {
        Expr::new(ExprKind::Literal(literal), self.span)
    }

    #[must_use]
    pub fn variable(self, name: impl Into<Symbol>) -> Expr {
        Expr::new(ExprKind::Var(name.into()), self.span)
    }

    #[must_use]
    pub fn binary(self, op: BinOp, left: Expr, right: Expr) -> Expr {
        Expr::new(
            ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            self.span,
        )
    }

    #[must_use]
    pub fn unary(self, op: UnOp, operand: Expr) -> Expr {
        Expr::new(
            ExprKind::Unary {
                op,
                operand: Box::new(operand),
            },
            self.span,
        )
    }

    #[must_use]
    pub fn builtin_call(self, id: BuiltinId, args: Vec<Expr>) -> Expr {
        self.call(Callee::Builtin(id), args)
    }

    #[must_use]
    pub fn user_call(self, name: impl Into<Symbol>, args: Vec<Expr>) -> Expr {
        self.call(Callee::User(name.into()), args)
    }

    #[must_use]
    pub fn call(self, callee: Callee, args: Vec<Expr>) -> Expr {
        Expr::new(ExprKind::Call { callee, args }, self.span)
    }

    #[must_use]
    pub fn nil_check(self, value: Expr) -> Expr {
        Expr::new(ExprKind::NilCheck(Box::new(value)), self.span)
    }

    #[must_use]
    pub fn expression(self, expression: Expr) -> Stmt {
        Stmt::new(StmtKind::Expr(expression), self.span)
    }

    #[must_use]
    pub fn let_value(self, name: impl Into<Symbol>, init: Expr) -> Stmt {
        Stmt::new(
            StmtKind::Let {
                name: name.into(),
                ty: None,
                init,
            },
            self.span,
        )
    }

    #[must_use]
    pub fn return_value(self, value: Option<Expr>) -> Stmt {
        Stmt::new(StmtKind::Return(value), self.span)
    }
}

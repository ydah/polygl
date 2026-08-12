use polygl_builtins::RuntimeOp;
use polygl_span::Span;
use polygl_types::Type;

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

impl Expr {
    #[must_use]
    pub const fn new(kind: ExprKind, ty: Type, span: Span) -> Self {
        Self { kind, ty, span }
    }

    #[must_use]
    pub fn effects(&self) -> EffectSet {
        match &self.kind {
            ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Constant(_) => EffectSet::PURE,
            ExprKind::Uniform(_) => EffectSet::READS_RUNTIME,
            ExprKind::Binary { op, left, right } => {
                let operation = if matches!(
                    op,
                    BinaryOp::IntegerDivide
                        | BinaryOp::FloorRemainder
                        | BinaryOp::TruncatingRemainder
                ) {
                    EffectSet::MAY_TRAP
                } else {
                    EffectSet::PURE
                };
                operation.union(left.effects()).union(right.effects())
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::ArrayLength(operand)
            | ExprKind::IsNil(operand)
            | ExprKind::IsFalsy(operand) => operand.effects(),
            ExprKind::Call { args, .. } => args
                .iter()
                .fold(EffectSet::WRITES_RUNTIME, |effects, argument| {
                    effects.union(argument.effects())
                }),
            ExprKind::Index { base, index } => EffectSet::MAY_TRAP
                .union(base.effects())
                .union(index.effects()),
            ExprKind::Field { base, .. } => EffectSet::MAY_TRAP.union(base.effects()),
            ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
                items.iter().fold(EffectSet::ALLOCATES, |effects, item| {
                    effects.union(item.effects())
                })
            }
            ExprKind::Map(entries) => {
                entries.iter().fold(EffectSet::ALLOCATES, |effects, entry| {
                    effects
                        .union(entry.key.effects())
                        .union(entry.value.effects())
                })
            }
            ExprKind::Struct { fields, .. } => {
                fields.iter().fold(EffectSet::ALLOCATES, |effects, field| {
                    effects.union(field.value.effects())
                })
            }
        }
    }

    #[must_use]
    pub fn is_trivially_pure(&self) -> bool {
        self.effects().is_pure()
    }
}

/// Conservative expression effects used by transformations that may remove or
/// reorder evaluation. Unknown calls are deliberately treated as writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectSet(u8);

impl EffectSet {
    pub const PURE: Self = Self(0);
    pub const MAY_TRAP: Self = Self(1 << 0);
    pub const READS_RUNTIME: Self = Self(1 << 1);
    pub const WRITES_RUNTIME: Self = Self(1 << 2);
    pub const ALLOCATES: Self = Self(1 << 3);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, effect: Self) -> bool {
        self.0 & effect.0 == effect.0
    }

    #[must_use]
    pub const fn is_pure(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Variable(String),
    Constant(String),
    Uniform(String),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Call {
        target: CallTarget,
        args: Vec<Expr>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Field {
        base: Box<Expr>,
        field: String,
    },
    ArrayLength(Box<Expr>),
    Array(Vec<Expr>),
    Map(Vec<MapEntry>),
    Struct {
        name: String,
        fields: Vec<FieldInit>,
    },
    Vector {
        size: u8,
        args: Vec<Expr>,
    },
    IsNil(Box<Expr>),
    IsFalsy(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CallTarget {
    Function(String),
    Runtime(RuntimeOp),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapEntry {
    pub key: Expr,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldInit {
    pub name: String,
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
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    IntegerDivide,
    FloatDivide,
    FloorRemainder,
    TruncatingRemainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    StringConcat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};
    use polygl_types::Type;

    use super::{BinaryOp, CallTarget, EffectSet, Expr, ExprKind, Literal};

    fn expression(kind: ExprKind) -> Expr {
        let source = SourceFile::new(SourceId::new(1), "effects.rb", "x");
        Expr::new(kind, Type::Int, source.span(0, 1).unwrap())
    }

    fn int(value: i32) -> Expr {
        expression(ExprKind::Literal(Literal::Int(value)))
    }

    #[test]
    fn classifies_traps_runtime_access_and_allocation_conservatively() {
        let add = expression(ExprKind::Binary {
            op: BinaryOp::Add,
            left: Box::new(int(1)),
            right: Box::new(int(2)),
        });
        assert!(add.effects().is_pure());

        let divide = expression(ExprKind::Binary {
            op: BinaryOp::IntegerDivide,
            left: Box::new(int(1)),
            right: Box::new(int(0)),
        });
        assert!(divide.effects().contains(EffectSet::MAY_TRAP));
        assert!(!divide.is_trivially_pure());

        let call = expression(ExprKind::Call {
            target: CallTarget::Function("unknown".to_owned()),
            args: vec![add],
        });
        assert!(call.effects().contains(EffectSet::WRITES_RUNTIME));

        let allocation = expression(ExprKind::Array(vec![int(1)]));
        assert!(allocation.effects().contains(EffectSet::ALLOCATES));
    }
}

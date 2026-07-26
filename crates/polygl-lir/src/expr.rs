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
    pub const fn is_trivially_pure(&self) -> bool {
        matches!(
            self.kind,
            ExprKind::Literal(_)
                | ExprKind::Variable(_)
                | ExprKind::Constant(_)
                | ExprKind::Uniform(_)
        )
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

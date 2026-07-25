use crate::{BinOp, Callee, Expr, ExprKind, Literal, UnOp};

use super::Dumper;

impl Dumper {
    pub(super) fn expression(&self, expression: &Expr) -> String {
        match &expression.kind {
            ExprKind::Literal(literal) => literal_text(literal),
            ExprKind::Var(name) => name.to_string(),
            ExprKind::Binary { op, left, right } => format!(
                "({} {} {})",
                self.expression(left),
                binary_name(*op),
                self.expression(right)
            ),
            ExprKind::Unary { op, operand } => {
                format!("({}{})", unary_name(*op), self.expression(operand))
            }
            ExprKind::Call { callee, args } => {
                let callee = match callee {
                    Callee::User(name) => name.to_string(),
                    Callee::Builtin(id) => format!("builtin#{}", id.raw()),
                };
                let args = args
                    .iter()
                    .map(|arg| self.expression(arg))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{callee}({args})")
            }
            ExprKind::Index { base, index } => {
                format!("{}[{}]", self.expression(base), self.expression(index))
            }
            ExprKind::Field { base, field } => {
                format!("{}.{}", self.expression(base), field)
            }
            ExprKind::Array(items) => self.expression_list("[", items, "]"),
            ExprKind::Map(entries) => {
                let entries = entries
                    .iter()
                    .map(|entry| {
                        format!(
                            "{}: {}",
                            self.expression(&entry.key),
                            self.expression(&entry.value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{{{entries}}}")
            }
            ExprKind::Struct { name, fields } => {
                let fields = fields
                    .iter()
                    .map(|field| format!("{}: {}", field.name, self.expression(&field.value)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{name} {{ {fields} }}")
            }
            ExprKind::Vector { size, args } => {
                format!("vec{size}{}", self.expression_list("(", args, ")"))
            }
            ExprKind::NilCheck(value) => format!("nil?({})", self.expression(value)),
            ExprKind::FalsyCheck(value) => format!("falsy?({})", self.expression(value)),
        }
    }

    fn expression_list(&self, open: &str, items: &[Expr], close: &str) -> String {
        let items = items
            .iter()
            .map(|item| self.expression(item))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{open}{items}{close}")
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Int(value) => value.to_string(),
        Literal::Float(value) => format!("{value:?}"),
        Literal::Bool(value) => value.to_string(),
        Literal::Str(value) => format!("{value:?}"),
        Literal::None => "none".to_owned(),
    }
}

const fn binary_name(operator: BinOp) -> &'static str {
    match operator {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::DivInt => "/int",
        BinOp::DivFloat => "/float",
        BinOp::Rem => "%",
        BinOp::Eq => "==",
        BinOp::NotEq => "!=",
        BinOp::Less => "<",
        BinOp::LessEq => "<=",
        BinOp::Greater => ">",
        BinOp::GreaterEq => ">=",
        BinOp::And => "and",
        BinOp::Or => "or",
        BinOp::StrConcat => "str++",
    }
}

const fn unary_name(operator: UnOp) -> &'static str {
    match operator {
        UnOp::Neg => "-",
        UnOp::Not => "not ",
    }
}

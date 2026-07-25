use polygl_hir::{BinOp, Expr, ExprKind, UnOp};
use ruby_prism::Node;

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_binary(
        &mut self,
        op: BinOp,
        left: &Node<'_>,
        right: &Node<'_>,
        span: polygl_span::Span,
    ) -> Option<Expr> {
        let left = self.lower_expression(left)?;
        let right = self.lower_expression(right)?;
        Some(Expr::new(
            ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        ))
    }

    pub(crate) fn lower_condition(&mut self, node: &Node<'_>) -> Option<Expr> {
        let span = self.span(node.location());
        if let Some(and) = node.as_and_node() {
            let left = self.lower_condition(&and.left())?;
            let right = self.lower_condition(&and.right())?;
            return Some(binary(BinOp::And, left, right, span));
        }
        if let Some(or) = node.as_or_node() {
            let left = self.lower_condition(&or.left())?;
            let right = self.lower_condition(&or.right())?;
            return Some(binary(BinOp::Or, left, right, span));
        }
        if let Some(call) = node.as_call_node()
            && self.name(call.name().as_slice()) == "!"
            && call.block().is_none()
            && call.arguments().is_none()
            && let Some(receiver) = call.receiver()
        {
            let operand = self.lower_condition(&receiver)?;
            return Some(Expr::new(
                ExprKind::Unary {
                    op: UnOp::Not,
                    operand: Box::new(operand),
                },
                span,
            ));
        }
        let expression = self.lower_expression(node)?;
        Some(self.truthy(expression))
    }

    pub(crate) fn truthy(&self, expression: Expr) -> Expr {
        let span = expression.span;
        let falsy = Expr::new(ExprKind::FalsyCheck(Box::new(expression)), span);
        Expr::new(
            ExprKind::Unary {
                op: UnOp::Not,
                operand: Box::new(falsy),
            },
            span,
        )
    }
}

pub(crate) fn binary_operator(name: &str) -> Option<BinOp> {
    match name {
        "+" => Some(BinOp::Add),
        "-" => Some(BinOp::Sub),
        "*" => Some(BinOp::Mul),
        "/" => Some(BinOp::DivInt),
        "%" => Some(BinOp::RemFloor),
        "==" => Some(BinOp::Eq),
        "!=" => Some(BinOp::NotEq),
        "<" => Some(BinOp::Less),
        "<=" => Some(BinOp::LessEq),
        ">" => Some(BinOp::Greater),
        ">=" => Some(BinOp::GreaterEq),
        _ => None,
    }
}

pub(crate) fn unary_operator(name: &str) -> Option<UnOp> {
    match name {
        "-@" => Some(UnOp::Neg),
        _ => None,
    }
}

fn binary(op: BinOp, left: Expr, right: Expr, span: polygl_span::Span) -> Expr {
    Expr::new(
        ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
        span,
    )
}

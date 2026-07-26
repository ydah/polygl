use polygl_types::Type;

use crate::{
    BinaryOp, Block, Domain, Expr, ExprKind, Literal, Module, Place, PlaceKind, Statement,
    StatementKind, UnaryOp,
};

pub(crate) fn optimize_module(module: &mut Module) {
    for constant in &mut module.constants {
        optimize_expression(&mut constant.value, constant.domain);
    }
    for function in &mut module.functions {
        optimize_block(&mut function.body, function.domain);
    }
    for entry in &mut module.entries {
        optimize_block(&mut entry.body, entry.domain);
    }
}

fn optimize_block(block: &mut Block, domain: Domain) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                optimize_expression(init, domain);
            }
            StatementKind::Assign { target, value } => {
                optimize_place(target, domain);
                optimize_expression(value, domain);
            }
            StatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                optimize_expression(condition, domain);
                optimize_block(then_block, domain);
                if let Some(else_block) = else_block {
                    optimize_block(else_block, domain);
                }
            }
            StatementKind::While { condition, body } => {
                optimize_expression(condition, domain);
                optimize_block(body, domain);
            }
            StatementKind::For { range, body, .. } => {
                optimize_expression(&mut range.start, domain);
                optimize_expression(&mut range.end, domain);
                optimize_block(body, domain);
            }
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    optimize_expression(value, domain);
                }
            }
            StatementKind::Break | StatementKind::Continue => {}
        }
    }
    remove_dead_statements(&mut block.statements);
}

fn optimize_place(place: &mut Place, domain: Domain) {
    match &mut place.kind {
        PlaceKind::Variable(_) => {}
        PlaceKind::Index { base, index } => {
            optimize_expression(base, domain);
            optimize_expression(index, domain);
        }
        PlaceKind::Field { base, .. } => optimize_expression(base, domain),
    }
}

fn optimize_expression(expression: &mut Expr, domain: Domain) {
    match &mut expression.kind {
        ExprKind::Literal(_)
        | ExprKind::Variable(_)
        | ExprKind::Constant(_)
        | ExprKind::Uniform(_) => {}
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            optimize_expression(left, domain);
            optimize_expression(right, domain);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Field { base: operand, .. }
        | ExprKind::ArrayLength(operand)
        | ExprKind::IsNil(operand)
        | ExprKind::IsFalsy(operand) => optimize_expression(operand, domain),
        ExprKind::Call { args, .. } | ExprKind::Array(args) | ExprKind::Vector { args, .. } => {
            for argument in args {
                optimize_expression(argument, domain);
            }
        }
        ExprKind::Map(entries) => {
            for entry in entries {
                optimize_expression(&mut entry.key, domain);
                optimize_expression(&mut entry.value, domain);
            }
        }
        ExprKind::Struct { fields, .. } => {
            for field in fields {
                optimize_expression(&mut field.value, domain);
            }
        }
    }
    fold_expression(expression, domain);
}

fn fold_expression(expression: &mut Expr, domain: Domain) {
    let folded = match &expression.kind {
        ExprKind::Binary { op, left, right } => {
            fold_binary(*op, &left.kind, &right.kind, &expression.ty, domain)
        }
        ExprKind::Unary { op, operand } => fold_unary(*op, &operand.kind, domain),
        ExprKind::IsNil(value) => match &value.kind {
            ExprKind::Literal(Literal::None) => Some(Literal::Bool(true)),
            ExprKind::Literal(_) => Some(Literal::Bool(false)),
            _ => None,
        },
        ExprKind::IsFalsy(value) => match &value.kind {
            ExprKind::Literal(Literal::None | Literal::Bool(false)) => Some(Literal::Bool(true)),
            ExprKind::Literal(_) => Some(Literal::Bool(false)),
            _ => None,
        },
        _ => None,
    };
    if let Some(literal) = folded {
        expression.kind = ExprKind::Literal(literal);
    }
}

fn remove_dead_statements(statements: &mut Vec<Statement>) {
    statements.retain(|statement| {
        !matches!(&statement.kind, StatementKind::Expr(expression) if expression.is_trivially_pure())
    });
}

fn fold_binary(
    operator: BinaryOp,
    left: &ExprKind,
    right: &ExprKind,
    result_type: &Type,
    domain: Domain,
) -> Option<Literal> {
    let (ExprKind::Literal(left), ExprKind::Literal(right)) = (left, right) else {
        return None;
    };
    match (operator, left, right) {
        (BinaryOp::Add, Literal::Int(left), Literal::Int(right)) => {
            left.checked_add(*right).map(Literal::Int)
        }
        (BinaryOp::Subtract, Literal::Int(left), Literal::Int(right)) => {
            left.checked_sub(*right).map(Literal::Int)
        }
        (BinaryOp::Multiply, Literal::Int(left), Literal::Int(right)) => {
            left.checked_mul(*right).map(Literal::Int)
        }
        (BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply, left, right)
            if result_type == &Type::Float && domain == Domain::Host =>
        {
            let left = number_as_float(left)?;
            let right = number_as_float(right)?;
            finite_float(match operator {
                BinaryOp::Add => left + right,
                BinaryOp::Subtract => left - right,
                BinaryOp::Multiply => left * right,
                _ => unreachable!("guard restricts the operator"),
            })
        }
        (BinaryOp::StringConcat, Literal::Str(left), Literal::Str(right)) => {
            Some(Literal::Str(format!("{left}{right}")))
        }
        (BinaryOp::Equal, left, right)
            if domain == Domain::Host || !contains_float(left, right) =>
        {
            Some(Literal::Bool(left == right))
        }
        (BinaryOp::NotEqual, left, right)
            if domain == Domain::Host || !contains_float(left, right) =>
        {
            Some(Literal::Bool(left != right))
        }
        (BinaryOp::Less, Literal::Int(left), Literal::Int(right)) => {
            Some(Literal::Bool(left < right))
        }
        (BinaryOp::LessEqual, Literal::Int(left), Literal::Int(right)) => {
            Some(Literal::Bool(left <= right))
        }
        (BinaryOp::Greater, Literal::Int(left), Literal::Int(right)) => {
            Some(Literal::Bool(left > right))
        }
        (BinaryOp::GreaterEqual, Literal::Int(left), Literal::Int(right)) => {
            Some(Literal::Bool(left >= right))
        }
        (BinaryOp::Less, Literal::Float(left), Literal::Float(right)) if domain == Domain::Host => {
            Some(Literal::Bool(left < right))
        }
        (BinaryOp::LessEqual, Literal::Float(left), Literal::Float(right))
            if domain == Domain::Host =>
        {
            Some(Literal::Bool(left <= right))
        }
        (BinaryOp::Greater, Literal::Float(left), Literal::Float(right))
            if domain == Domain::Host =>
        {
            Some(Literal::Bool(left > right))
        }
        (BinaryOp::GreaterEqual, Literal::Float(left), Literal::Float(right))
            if domain == Domain::Host =>
        {
            Some(Literal::Bool(left >= right))
        }
        (BinaryOp::And, Literal::Bool(left), Literal::Bool(right)) => {
            Some(Literal::Bool(*left && *right))
        }
        (BinaryOp::Or, Literal::Bool(left), Literal::Bool(right)) => {
            Some(Literal::Bool(*left || *right))
        }
        _ => None,
    }
}

fn fold_unary(operator: UnaryOp, operand: &ExprKind, domain: Domain) -> Option<Literal> {
    let ExprKind::Literal(operand) = operand else {
        return None;
    };
    match (operator, operand) {
        (UnaryOp::Negate, Literal::Int(value)) => value.checked_neg().map(Literal::Int),
        (UnaryOp::Negate, Literal::Float(value)) if domain == Domain::Host => finite_float(-value),
        (UnaryOp::Not, Literal::Bool(value)) => Some(Literal::Bool(!value)),
        _ => None,
    }
}

const fn contains_float(left: &Literal, right: &Literal) -> bool {
    matches!(left, Literal::Float(_)) || matches!(right, Literal::Float(_))
}

fn finite_float(value: f64) -> Option<Literal> {
    value.is_finite().then_some(Literal::Float(value))
}

const fn number_as_float(literal: &Literal) -> Option<f64> {
    match literal {
        Literal::Int(value) => Some(*value as f64),
        Literal::Float(value) => Some(*value),
        Literal::Bool(_) | Literal::Str(_) | Literal::None => None,
    }
}

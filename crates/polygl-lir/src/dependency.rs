use crate::{Block, CallTarget, Expr, ExprKind, Place, PlaceKind, StatementKind};

#[derive(Default)]
pub(crate) struct NamedDependencies {
    pub functions: Vec<String>,
    pub constants: Vec<String>,
}

#[derive(Clone, Copy)]
enum Work<'lir> {
    Block(&'lir Block),
    Expr(&'lir Expr),
    Place(&'lir Place),
}

pub(crate) fn block_dependencies(block: &Block) -> NamedDependencies {
    collect(Work::Block(block))
}

pub(crate) fn expression_dependencies(expression: &Expr) -> NamedDependencies {
    collect(Work::Expr(expression))
}

fn collect(root: Work<'_>) -> NamedDependencies {
    let mut result = NamedDependencies::default();
    let mut work = vec![root];
    while let Some(node) = work.pop() {
        match node {
            Work::Block(block) => {
                for statement in block.statements.iter().rev() {
                    match &statement.kind {
                        StatementKind::Let { init, .. } | StatementKind::Expr(init) => {
                            work.push(Work::Expr(init));
                        }
                        StatementKind::Assign { target, value } => {
                            work.push(Work::Expr(value));
                            work.push(Work::Place(target));
                        }
                        StatementKind::If {
                            condition,
                            then_block,
                            else_block,
                        } => {
                            if let Some(else_block) = else_block {
                                work.push(Work::Block(else_block));
                            }
                            work.push(Work::Block(then_block));
                            work.push(Work::Expr(condition));
                        }
                        StatementKind::While { condition, body } => {
                            work.push(Work::Block(body));
                            work.push(Work::Expr(condition));
                        }
                        StatementKind::For { range, body, .. } => {
                            work.push(Work::Block(body));
                            work.push(Work::Expr(&range.end));
                            work.push(Work::Expr(&range.start));
                        }
                        StatementKind::Return(value) => {
                            if let Some(value) = value {
                                work.push(Work::Expr(value));
                            }
                        }
                        StatementKind::Break | StatementKind::Continue => {}
                    }
                }
            }
            Work::Place(place) => match &place.kind {
                PlaceKind::Variable(_) => {}
                PlaceKind::Index { base, index } => {
                    work.push(Work::Expr(index));
                    work.push(Work::Expr(base));
                }
                PlaceKind::Field { base, .. } => work.push(Work::Expr(base)),
            },
            Work::Expr(expression) => match &expression.kind {
                ExprKind::Call { target, args } => {
                    if let CallTarget::Function(name) = target {
                        result.functions.push(name.clone());
                    }
                    work.extend(args.iter().rev().map(Work::Expr));
                }
                ExprKind::Constant(name) => result.constants.push(name.clone()),
                ExprKind::Binary { left, right, .. }
                | ExprKind::Index {
                    base: left,
                    index: right,
                } => {
                    work.push(Work::Expr(right));
                    work.push(Work::Expr(left));
                }
                ExprKind::Unary { operand, .. }
                | ExprKind::Field { base: operand, .. }
                | ExprKind::ArrayLength(operand)
                | ExprKind::IsNil(operand)
                | ExprKind::IsFalsy(operand) => work.push(Work::Expr(operand)),
                ExprKind::Array(items) | ExprKind::Vector { args: items, .. } => {
                    work.extend(items.iter().rev().map(Work::Expr));
                }
                ExprKind::Map(entries) => {
                    for entry in entries.iter().rev() {
                        work.push(Work::Expr(&entry.value));
                        work.push(Work::Expr(&entry.key));
                    }
                }
                ExprKind::Struct { fields, .. } => {
                    work.extend(fields.iter().rev().map(|field| Work::Expr(&field.value)));
                }
                ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Uniform(_) => {}
            },
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};
    use polygl_types::Type;

    use crate::{Block, CallTarget, Expr, ExprKind, Statement, StatementKind};

    use super::block_dependencies;

    #[test]
    fn walks_deep_dependencies_iteratively_in_source_order() {
        let source = SourceFile::new(SourceId::new(1), "deep", "x");
        let span = source.span(0, 1).unwrap();
        let mut expression = Expr::new(ExprKind::Constant("first".to_owned()), Type::Int, span);
        for _ in 0..4_096 {
            expression = Expr::new(
                ExprKind::Unary {
                    op: crate::UnaryOp::Negate,
                    operand: Box::new(expression),
                },
                Type::Int,
                span,
            );
        }
        let call = Expr::new(
            ExprKind::Call {
                target: CallTarget::Function("second".to_owned()),
                args: vec![expression],
            },
            Type::Int,
            span,
        );
        let block = Block {
            statements: vec![Statement::new(StatementKind::Expr(call), span)],
            span,
        };

        let dependencies = block_dependencies(&block);
        assert_eq!(dependencies.functions, ["second"]);
        assert_eq!(dependencies.constants, ["first"]);
    }
}

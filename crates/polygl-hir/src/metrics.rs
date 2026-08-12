use std::collections::BTreeSet;

use crate::{Block, EntryPointKind, Expr, ExprKind, Item, Module, Place, PlaceKind, StmtKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModuleMetrics {
    pub item_count: usize,
    pub function_count: usize,
    pub shader_count: usize,
    pub syntax_node_count: usize,
    pub max_syntax_depth: usize,
}

#[derive(Clone, Copy)]
enum Work<'module> {
    Block(&'module Block, usize),
    Expr(&'module Expr, usize),
    Place(&'module Place, usize),
}

/// Counts source-shaped HIR with an explicit work stack so budget validation
/// itself remains safe for deeply nested input.
#[must_use]
pub fn module_metrics(module: &Module) -> ModuleMetrics {
    let mut metrics = ModuleMetrics {
        item_count: module.items.len(),
        syntax_node_count: 1 + module.items.len(),
        max_syntax_depth: usize::from(!module.items.is_empty()),
        ..ModuleMetrics::default()
    };
    let mut work = Vec::new();
    let mut shaders = BTreeSet::new();
    for item in &module.items {
        match item {
            Item::Function(function) => {
                metrics.function_count += 1;
                work.push(Work::Block(&function.body, 2));
            }
            Item::Struct(definition) => {
                metrics.function_count += definition.methods.len();
                metrics.syntax_node_count += definition.fields.len() + definition.methods.len();
                for method in &definition.methods {
                    work.push(Work::Block(&method.body, 3));
                }
            }
            Item::Const(constant) => work.push(Work::Expr(&constant.value, 2)),
            Item::Entry(entry) => {
                match &entry.kind {
                    EntryPointKind::Vertex(name) | EntryPointKind::Fragment(name) => {
                        shaders.insert(name.as_str());
                    }
                    EntryPointKind::Setup | EntryPointKind::Frame | EntryPointKind::OnEvent => {}
                }
                work.push(Work::Block(&entry.body, 2));
            }
        }
    }

    while let Some(node) = work.pop() {
        let depth = match node {
            Work::Block(block, depth) => {
                metrics.syntax_node_count += 1 + block.statements.len();
                for statement in &block.statements {
                    let child_depth = depth + 1;
                    match &statement.kind {
                        StmtKind::Let { init, .. } | StmtKind::Expr(init) => {
                            work.push(Work::Expr(init, child_depth));
                        }
                        StmtKind::Assign { target, value } => {
                            work.push(Work::Place(target, child_depth));
                            work.push(Work::Expr(value, child_depth));
                        }
                        StmtKind::If {
                            condition,
                            then_block,
                            else_block,
                        } => {
                            work.push(Work::Expr(condition, child_depth));
                            work.push(Work::Block(then_block, child_depth));
                            if let Some(else_block) = else_block {
                                work.push(Work::Block(else_block, child_depth));
                            }
                        }
                        StmtKind::While { condition, body } => {
                            work.push(Work::Expr(condition, child_depth));
                            work.push(Work::Block(body, child_depth));
                        }
                        StmtKind::For { range, body, .. } => {
                            work.push(Work::Expr(&range.start, child_depth));
                            work.push(Work::Expr(&range.end, child_depth));
                            work.push(Work::Block(body, child_depth));
                        }
                        StmtKind::Return(value) => {
                            if let Some(value) = value {
                                work.push(Work::Expr(value, child_depth));
                            }
                        }
                        StmtKind::Break | StmtKind::Continue => {}
                    }
                }
                depth
            }
            Work::Place(place, depth) => {
                metrics.syntax_node_count += 1;
                match &place.kind {
                    PlaceKind::Var(_) => {}
                    PlaceKind::Index { base, index } => {
                        work.push(Work::Expr(base, depth + 1));
                        work.push(Work::Expr(index, depth + 1));
                    }
                    PlaceKind::Field { base, .. } => {
                        work.push(Work::Expr(base, depth + 1));
                    }
                }
                depth
            }
            Work::Expr(expression, depth) => {
                metrics.syntax_node_count += 1;
                let child_depth = depth + 1;
                match &expression.kind {
                    ExprKind::Literal(_) | ExprKind::Var(_) | ExprKind::Uniform { .. } => {}
                    ExprKind::Binary { left, right, .. }
                    | ExprKind::Index {
                        base: left,
                        index: right,
                    } => {
                        work.push(Work::Expr(left, child_depth));
                        work.push(Work::Expr(right, child_depth));
                    }
                    ExprKind::Unary { operand, .. }
                    | ExprKind::Field { base: operand, .. }
                    | ExprKind::ArrayLength(operand)
                    | ExprKind::NilCheck(operand)
                    | ExprKind::FalsyCheck(operand) => {
                        work.push(Work::Expr(operand, child_depth));
                    }
                    ExprKind::Call { args, .. }
                    | ExprKind::Array(args)
                    | ExprKind::Vector { args, .. } => {
                        for argument in args {
                            work.push(Work::Expr(argument, child_depth));
                        }
                    }
                    ExprKind::Map(entries) => {
                        for entry in entries {
                            work.push(Work::Expr(&entry.key, child_depth));
                            work.push(Work::Expr(&entry.value, child_depth));
                        }
                    }
                    ExprKind::Struct { fields, .. } => {
                        for field in fields {
                            work.push(Work::Expr(&field.value, child_depth));
                        }
                    }
                }
                depth
            }
        };
        metrics.max_syntax_depth = metrics.max_syntax_depth.max(depth);
    }
    metrics.shader_count = shaders.len();
    metrics
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};

    use crate::{HirBuilder, Item, UnOp};

    use super::module_metrics;

    #[test]
    fn measures_deep_expressions_without_recursive_walking() {
        let source = SourceFile::new(SourceId::new(1), "deep.rb", "x");
        let span = source.span(0, 1).unwrap();
        let builder = HirBuilder::new(span);
        let mut expression = builder.int(1);
        for _ in 0..2_048 {
            expression = builder.unary(UnOp::Neg, expression);
        }
        let module = builder.module(vec![Item::Const(crate::ConstDef {
            name: "DEEP".into(),
            ty: None,
            value: expression,
            span,
        })]);

        let metrics = module_metrics(&module);
        assert_eq!(metrics.item_count, 1);
        assert_eq!(metrics.function_count, 0);
        assert!(metrics.max_syntax_depth >= 2_048);
        assert!(metrics.syntax_node_count >= 2_050);
    }
}

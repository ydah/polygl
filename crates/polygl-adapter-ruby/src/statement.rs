use polygl_hir::{Block, Expr, ExprKind, Literal, Place, PlaceKind, Stmt, StmtKind, Symbol};
use ruby_prism::{ArgumentsNode, IfNode, Node};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_statement(&mut self, node: &Node<'_>) -> Option<Stmt> {
        let span = self.span(node.location());
        let kind = if let Some(write) = node.as_local_variable_write_node() {
            let name = self.name(write.name().as_slice());
            let value = self.lower_expression(&write.value())?;
            if self.declared.insert(name.clone()) {
                let ty = self.annotation_for(&name, write.location());
                StmtKind::Let {
                    name: Symbol::new(name),
                    ty,
                    init: value,
                }
            } else {
                StmtKind::Assign {
                    target: Place {
                        kind: PlaceKind::Var(Symbol::new(name)),
                        span: self.span(write.name_loc()),
                    },
                    value,
                }
            }
        } else if let Some(if_node) = node.as_if_node() {
            return self.lower_if(&if_node);
        } else if let Some(while_node) = node.as_while_node() {
            if while_node.is_begin_modifier() {
                self.unsupported(
                    node,
                    "post-test Ruby loops are outside Common Core",
                    "use a conventional `while condition` loop",
                );
                return None;
            }
            let condition = self.lower_condition(&while_node.predicate())?;
            self.loop_depth += 1;
            let body = self.lower_nested_statements(while_node.statements(), span);
            self.loop_depth -= 1;
            StmtKind::While { condition, body }
        } else if let Some(return_node) = node.as_return_node() {
            StmtKind::Return(self.lower_optional_value(return_node.arguments(), node)?)
        } else if let Some(break_node) = node.as_break_node() {
            if break_node.arguments().is_some() {
                self.unsupported(
                    node,
                    "break values are outside Common Core",
                    "use `break` without a value",
                );
                return None;
            }
            if self.loop_depth == 0 {
                self.unsupported(
                    node,
                    "`break` is only valid inside a loop",
                    "move `break` into a `while` loop",
                );
                return None;
            }
            StmtKind::Break
        } else if let Some(next_node) = node.as_next_node() {
            if next_node.arguments().is_some() {
                self.unsupported(
                    node,
                    "next values are outside Common Core",
                    "use `next` without a value",
                );
                return None;
            }
            if self.loop_depth == 0 {
                self.unsupported(
                    node,
                    "`next` is only valid inside a loop",
                    "move `next` into a `while` loop",
                );
                return None;
            }
            StmtKind::Continue
        } else {
            StmtKind::Expr(self.lower_expression(node)?)
        };
        Some(Stmt::new(kind, span))
    }

    fn lower_if(&mut self, if_node: &IfNode<'_>) -> Option<Stmt> {
        let span = self.span(if_node.location());
        let condition = self.lower_condition(&if_node.predicate())?;
        let then_block = self.lower_nested_statements(if_node.statements(), span);
        let else_block = if_node
            .subsequent()
            .and_then(|node| self.lower_subsequent(&node));
        Some(Stmt::new(
            StmtKind::If {
                condition,
                then_block,
                else_block,
            },
            span,
        ))
    }

    fn lower_subsequent(&mut self, node: &Node<'_>) -> Option<Block> {
        if let Some(else_node) = node.as_else_node() {
            let span = self.span(else_node.location());
            Some(self.lower_nested_statements(else_node.statements(), span))
        } else if let Some(if_node) = node.as_if_node() {
            let span = self.span(if_node.location());
            self.lower_if(&if_node).map(|statement| Block {
                statements: vec![statement],
                span,
            })
        } else {
            self.unsupported(
                node,
                "this Ruby conditional branch is outside Common Core",
                "use `elsif` or `else` with ordinary statements",
            );
            None
        }
    }

    fn lower_optional_value(
        &mut self,
        arguments: Option<ArgumentsNode<'_>>,
        node: &Node<'_>,
    ) -> Option<Option<Expr>> {
        let Some(arguments) = arguments else {
            return Some(None);
        };
        let values = arguments.arguments();
        if values.len() > 1 {
            self.unsupported(
                node,
                "multiple return values are outside Common Core",
                "return one value",
            );
            return None;
        }
        if let Some(value) = values.first() {
            self.lower_expression(&value).map(Some)
        } else {
            Some(Some(Expr::new(
                ExprKind::Literal(Literal::None),
                self.span(node.location()),
            )))
        }
    }
}

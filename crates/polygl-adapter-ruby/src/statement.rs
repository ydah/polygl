use polygl_hir::{
    Block, Expr, ExprKind, Literal, Place, PlaceKind, RangeExpr, Stmt, StmtKind, Symbol,
};
use ruby_prism::{ArgumentsNode, BlockNode, CallNode, IfNode, Node, RangeNode};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_statement(&mut self, node: &Node<'_>) -> Option<Vec<Stmt>> {
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
        } else if let Some(call) = node.as_call_node()
            && call.block().is_some()
        {
            return self.lower_block_call(&call);
        } else if let Some(call) = node.as_call_node()
            && call.is_attribute_write()
            && self.name(call.name().as_slice()) == "[]="
        {
            return self.lower_index_write(&call);
        } else if let Some(if_node) = node.as_if_node() {
            return self.lower_if(&if_node).map(|statement| vec![statement]);
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
        Some(vec![Stmt::new(kind, span)])
    }

    fn lower_index_write(&mut self, call: &CallNode<'_>) -> Option<Vec<Stmt>> {
        let node = call.as_node();
        let Some(receiver) = call.receiver() else {
            self.unsupported(
                &node,
                "index assignment requires a collection receiver",
                "write `collection[index] = value`",
            );
            return None;
        };
        let arguments = call
            .arguments()
            .map_or_else(Vec::new, |arguments| arguments.arguments().iter().collect());
        if arguments.len() != 2 {
            self.unsupported(
                &node,
                "index assignment requires one index and one value",
                "write `collection[index] = value`",
            );
            return None;
        }
        let base = self.lower_expression(&receiver)?;
        let index = self.lower_expression(&arguments[0])?;
        let value = self.lower_expression(&arguments[1])?;
        let span = self.span(call.location());
        Some(vec![Stmt::new(
            StmtKind::Assign {
                target: Place {
                    kind: PlaceKind::Index { base, index },
                    span,
                },
                value,
            },
            span,
        )])
    }

    fn lower_block_call(&mut self, call: &CallNode<'_>) -> Option<Vec<Stmt>> {
        let node = call.as_node();
        let name = self.name(call.name().as_slice());
        let Some(block) = call.block().and_then(|block| block.as_block_node()) else {
            self.unsupported_with_code(
                &node,
                "E0202",
                "escaping or forwarded Ruby blocks are outside Common Core",
                "replace the block value with a direct `times` or `each` loop",
            );
            return None;
        };
        if call
            .arguments()
            .is_some_and(|arguments| !arguments.arguments().is_empty())
        {
            self.unsupported_with_code(
                &node,
                "E0202",
                "block sugar does not accept ordinary call arguments",
                "move loop inputs into the receiver expression",
            );
            return None;
        }
        let Some(receiver) = call.receiver() else {
            self.unsupported_with_code(
                &node,
                "E0202",
                "only receiver-based `times` and `each` blocks are supported",
                "rewrite this block as a `while` loop or a plain function",
            );
            return None;
        };
        let parameter = self.lower_block_parameter(&block)?;
        match name.as_str() {
            "times" => self.lower_times_block(call, &block, &receiver, parameter),
            "each" => self.lower_each_block(call, &block, &receiver, parameter),
            _ => {
                self.unsupported_with_code(
                    &node,
                    "E0202",
                    "this Ruby block is not on the Common Core whitelist",
                    "rewrite the block as a `while` loop or move it into a plain function",
                );
                None
            }
        }
    }

    fn lower_times_block(
        &mut self,
        call: &CallNode<'_>,
        block: &BlockNode<'_>,
        receiver: &Node<'_>,
        parameter: Option<(String, polygl_span::Span)>,
    ) -> Option<Vec<Stmt>> {
        let end = self.lower_expression(receiver)?;
        let variable = parameter
            .map(|(name, _)| name)
            .unwrap_or_else(|| self.temporary("times_index"));
        let body = self.lower_block_body(block, &variable, None);
        let span = self.span(call.location());
        Some(vec![Stmt::new(
            StmtKind::For {
                variable: Symbol::new(variable),
                range: RangeExpr {
                    start: Expr::new(ExprKind::Literal(Literal::Int(0)), span),
                    end,
                    inclusive: false,
                    span,
                },
                body,
            },
            span,
        )])
    }

    fn lower_each_block(
        &mut self,
        call: &CallNode<'_>,
        block: &BlockNode<'_>,
        receiver: &Node<'_>,
        parameter: Option<(String, polygl_span::Span)>,
    ) -> Option<Vec<Stmt>> {
        if let Some(range) = range_receiver(receiver) {
            let Some(start) = range.left() else {
                self.unsupported_with_code(
                    &call.as_node(),
                    "E0202",
                    "beginless ranges cannot be expanded as Common Core loops",
                    "provide an explicit integer range start",
                );
                return None;
            };
            let Some(end) = range.right() else {
                self.unsupported_with_code(
                    &call.as_node(),
                    "E0202",
                    "endless ranges cannot be expanded as Common Core loops",
                    "provide an explicit integer range end",
                );
                return None;
            };
            let variable = parameter
                .map(|(name, _)| name)
                .unwrap_or_else(|| self.temporary("range_value"));
            let body = self.lower_block_body(block, &variable, None);
            let span = self.span(call.location());
            return Some(vec![Stmt::new(
                StmtKind::For {
                    variable: Symbol::new(variable),
                    range: RangeExpr {
                        start: self.lower_expression(&start)?,
                        end: self.lower_expression(&end)?,
                        inclusive: !range.is_exclude_end(),
                        span: self.span(range.location()),
                    },
                    body,
                },
                span,
            )]);
        }

        let values = self.lower_expression(receiver)?;
        let values_span = values.span;
        let values_name = self.temporary("each_values");
        let index_name = self.temporary("each_index");
        let item_name = parameter
            .map(|(name, _)| name)
            .unwrap_or_else(|| self.temporary("each_value"));
        let values_expr = Expr::new(ExprKind::Var(Symbol::new(values_name.clone())), values_span);
        let index_expr = Expr::new(ExprKind::Var(Symbol::new(index_name.clone())), values_span);
        let item = Stmt::new(
            StmtKind::Let {
                name: Symbol::new(item_name.clone()),
                ty: None,
                init: Expr::new(
                    ExprKind::Index {
                        base: Box::new(values_expr.clone()),
                        index: Box::new(index_expr),
                    },
                    values_span,
                ),
            },
            values_span,
        );
        let body = self.lower_block_body(block, &item_name, Some(item));
        let span = self.span(call.location());
        Some(vec![
            Stmt::new(
                StmtKind::Let {
                    name: Symbol::new(values_name),
                    ty: None,
                    init: values,
                },
                values_span,
            ),
            Stmt::new(
                StmtKind::For {
                    variable: Symbol::new(index_name),
                    range: RangeExpr {
                        start: Expr::new(ExprKind::Literal(Literal::Int(0)), span),
                        end: Expr::new(ExprKind::ArrayLength(Box::new(values_expr)), span),
                        inclusive: false,
                        span,
                    },
                    body,
                },
                span,
            ),
        ])
    }

    fn lower_block_parameter(
        &mut self,
        block: &BlockNode<'_>,
    ) -> Option<Option<(String, polygl_span::Span)>> {
        let Some(parameters) = block.parameters() else {
            return Some(None);
        };
        let Some(parameters) = parameters.as_block_parameters_node() else {
            self.unsupported_with_code(
                &block.as_node(),
                "E0202",
                "numbered or forwarded block parameters are outside Common Core",
                "use zero or one explicit block parameter such as `|item|`",
            );
            return None;
        };
        if !parameters.locals().is_empty() {
            self.unsupported_with_code(
                &block.as_node(),
                "E0202",
                "block-local declarations are outside Common Core",
                "initialize the local inside the loop body",
            );
            return None;
        }
        let Some(parameters) = parameters.parameters() else {
            return Some(None);
        };
        if !parameters.optionals().is_empty()
            || parameters.rest().is_some()
            || !parameters.posts().is_empty()
            || !parameters.keywords().is_empty()
            || parameters.keyword_rest().is_some()
            || parameters.block().is_some()
            || parameters.requireds().len() > 1
        {
            self.unsupported_with_code(
                &block.as_node(),
                "E0202",
                "block sugar supports at most one required parameter",
                "use one parameter such as `|item|` and initialize other locals in the body",
            );
            return None;
        }
        let Some(parameter) = parameters.requireds().first() else {
            return Some(None);
        };
        let Some(parameter) = parameter.as_required_parameter_node() else {
            self.unsupported_with_code(
                &block.as_node(),
                "E0202",
                "this block parameter form is outside Common Core",
                "use one required parameter such as `|item|`",
            );
            return None;
        };
        Some(Some((
            self.name(parameter.name().as_slice()),
            self.span(parameter.location()),
        )))
    }

    fn lower_block_body(
        &mut self,
        block: &BlockNode<'_>,
        parameter: &str,
        prefix: Option<Stmt>,
    ) -> Block {
        let outer = self.declared.clone();
        self.declared.insert(parameter.to_owned());
        self.loop_depth += 1;
        let mut body = self.lower_body(block.body(), self.span(block.location()));
        self.loop_depth -= 1;
        self.declared = outer;
        if let Some(prefix) = prefix {
            body.statements.insert(0, prefix);
        }
        body
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

fn range_receiver<'pr>(node: &Node<'pr>) -> Option<RangeNode<'pr>> {
    if let Some(range) = node.as_range_node() {
        return Some(range);
    }
    let body = node.as_parentheses_node()?.body()?;
    let statements = body.as_statements_node()?;
    if statements.body().len() != 1 {
        return None;
    }
    statements.body().first()?.as_range_node()
}

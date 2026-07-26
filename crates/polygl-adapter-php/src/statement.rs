use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Assignment, AssignmentOperator, BinaryOperator, ClassLikeMemberSelector, Expression,
    For, Foreach, ForeachTarget, If, Statement, UnaryPostfixOperator, UnaryPrefixOperator,
    Variable, While, WhileBody,
};
use polygl_hir::{
    Block, Expr, ExprKind, Literal, Place, PlaceKind, RangeExpr, Stmt, StmtKind, Symbol,
};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_statement(&mut self, statement: &Statement<'_>) -> Option<Vec<Stmt>> {
        let span = self.span(statement.span());
        let kind = match statement {
            Statement::Expression(expression) => {
                if let Expression::Assignment(assignment) = expression.expression {
                    return self
                        .lower_assignment(assignment)
                        .map(|statement| vec![statement]);
                }
                StmtKind::Expr(self.lower_expression(expression.expression)?)
            }
            Statement::If(r#if) => return self.lower_if(r#if).map(|statement| vec![statement]),
            Statement::While(r#while) => {
                let condition = self.lower_expression(r#while.condition)?;
                self.loop_depth += 1;
                let body = self.lower_while_body(r#while);
                self.loop_depth -= 1;
                StmtKind::While { condition, body }
            }
            Statement::For(r#for) => return self.lower_for(r#for).map(|statement| vec![statement]),
            Statement::Foreach(foreach) => return self.lower_foreach(foreach),
            Statement::Return(r#return) => StmtKind::Return(
                r#return
                    .value
                    .and_then(|value| self.lower_expression(value)),
            ),
            Statement::Break(r#break) => {
                if r#break.level.is_some() || self.loop_depth == 0 {
                    self.unsupported(
                        statement.span(),
                        "break levels or break outside a loop are outside Common Core",
                        "use `break` without a level inside a loop",
                    );
                    return None;
                }
                StmtKind::Break
            }
            Statement::Continue(r#continue) => {
                if r#continue.level.is_some() || self.loop_depth == 0 {
                    self.unsupported(
                        statement.span(),
                        "continue levels or continue outside a loop are outside Common Core",
                        "use `continue` without a level inside a loop",
                    );
                    return None;
                }
                StmtKind::Continue
            }
            Statement::Block(block) => return Some(self.lower_block(block).statements),
            Statement::Noop(_) => return Some(Vec::new()),
            _ => {
                self.unsupported(
                    statement.span(),
                    "this PHP statement is outside Common Core",
                    "rewrite it using assignment, if, while, return, break, or continue",
                );
                return None;
            }
        };
        Some(vec![Stmt::new(kind, span)])
    }

    fn lower_assignment(&mut self, assignment: &Assignment<'_>) -> Option<Stmt> {
        if !matches!(assignment.operator, AssignmentOperator::Assign(_)) {
            self.unsupported(
                assignment.operator.span(),
                "compound PHP assignments are outside Common Core",
                "expand the operation into a plain assignment",
            );
            return None;
        }
        let span = self.span(assignment.span());
        if let Expression::Variable(Variable::Direct(variable)) = assignment.lhs {
            let name = self.variable_name(variable.name);
            if !self.declared.contains(&name) {
                let ty = self.annotation_for(&name, assignment.span());
                let value = self.lower_expression_with_expected(assignment.rhs, ty.as_ref())?;
                self.declared.insert(name.clone());
                return Some(Stmt::new(
                    StmtKind::Let {
                        name: Symbol::new(name),
                        ty,
                        init: value,
                    },
                    span,
                ));
            }
            let value = self.lower_expression(assignment.rhs)?;
            return Some(Stmt::new(
                StmtKind::Assign {
                    target: Place {
                        kind: PlaceKind::Var(Symbol::new(name)),
                        span: self.span(variable.span()),
                    },
                    value,
                },
                span,
            ));
        }
        let value = self.lower_expression(assignment.rhs)?;
        if let Expression::ArrayAccess(access) = assignment.lhs {
            return Some(Stmt::new(
                StmtKind::Assign {
                    target: Place {
                        kind: PlaceKind::Index {
                            base: self.lower_expression(access.array)?,
                            index: self.lower_expression(access.index)?,
                        },
                        span: self.span(access.span()),
                    },
                    value,
                },
                span,
            ));
        }
        if let Expression::Access(Access::Property(access)) = assignment.lhs {
            let ClassLikeMemberSelector::Identifier(field) = &access.property else {
                self.unsupported_with_code(
                    access.property.span(),
                    "E0203",
                    "dynamic PHP property names are outside Common Core",
                    "assign a directly named field",
                );
                return None;
            };
            let field_name = self.name(field.value);
            if !self.field_names.contains(&field_name) {
                self.unsupported_with_code(
                    field.span(),
                    "E0203",
                    "field assignment requires a field established by a constructor",
                    "establish the field once in `__construct` before assigning it",
                );
                return None;
            }
            let base = if matches!(
                access.object,
                Expression::Variable(Variable::Direct(variable)) if variable.name == b"$this"
            ) {
                if self.current_class.is_none() {
                    self.unsupported_with_code(
                        access.object.span(),
                        "E0203",
                        "`$this` is only available inside a Common Core class method",
                        "assign through an explicit instance variable",
                    );
                    return None;
                }
                Expr::new(
                    ExprKind::Var(Symbol::new("self")),
                    self.span(access.object.span()),
                )
            } else {
                self.lower_expression(access.object)?
            };
            return Some(Stmt::new(
                StmtKind::Assign {
                    target: Place {
                        kind: PlaceKind::Field {
                            base,
                            field: Symbol::new(field_name),
                        },
                        span: self.span(access.span()),
                    },
                    value,
                },
                span,
            ));
        }
        self.unsupported(
            assignment.lhs.span(),
            "this PHP assignment target is outside Common Core",
            "assign to a local, collection index, or declared class field",
        );
        None
    }

    fn lower_if(&mut self, r#if: &If<'_>) -> Option<Stmt> {
        let condition = self.lower_expression(r#if.condition)?;
        let then_block = self.lower_nested_statements(r#if.body.statements(), r#if.body.span());
        let mut else_block = r#if
            .body
            .else_statements()
            .map(|statements| self.lower_nested_statements(statements, r#if.body.span()));
        for (else_condition, statements) in r#if.body.else_if_clauses().into_iter().rev() {
            let condition = self.lower_expression(else_condition)?;
            let then_block = self.lower_nested_statements(statements, else_condition.span());
            let span = self.span(else_condition.span());
            let nested = Stmt::new(
                StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                },
                span,
            );
            else_block = Some(Block {
                statements: vec![nested],
                span,
            });
        }
        Some(Stmt::new(
            StmtKind::If {
                condition,
                then_block,
                else_block,
            },
            self.span(r#if.span()),
        ))
    }

    fn lower_while_body(&mut self, r#while: &While<'_>) -> Block {
        match &r#while.body {
            WhileBody::Statement(statement) => {
                self.lower_nested_statements(std::slice::from_ref(*statement), statement.span())
            }
            WhileBody::ColonDelimited(body) => {
                self.lower_nested_statements(body.statements.as_slice(), body.span())
            }
        }
    }

    fn lower_for(&mut self, r#for: &For<'_>) -> Option<Stmt> {
        let Some(initialization) = exactly_one(r#for.initializations.as_slice()) else {
            return self.invalid_for(r#for, "initialize exactly one loop variable");
        };
        let Expression::Assignment(initialization) = initialization else {
            return self.invalid_for(r#for, "initialize the loop with `$i = start`");
        };
        if !matches!(initialization.operator, AssignmentOperator::Assign(_)) {
            return self.invalid_for(r#for, "initialize the loop with a plain assignment");
        }
        let Expression::Variable(Variable::Direct(variable)) = initialization.lhs else {
            return self.invalid_for(r#for, "use a directly named loop variable");
        };
        let name = self.variable_name(variable.name);
        if self.declared.contains(&name) {
            return self.invalid_for(r#for, "use a fresh loop variable");
        }
        let start = self.lower_expression(initialization.rhs)?;

        let Some(condition) = exactly_one(r#for.conditions.as_slice()) else {
            return self.invalid_for(r#for, "write exactly one `<` or `<=` loop condition");
        };
        let Expression::Binary(condition) = condition else {
            return self.invalid_for(r#for, "compare the loop variable with `<` or `<=`");
        };
        if !is_direct_variable(condition.lhs, &name) {
            return self.invalid_for(r#for, "put the loop variable on the left of the comparison");
        }
        let inclusive = match condition.operator {
            BinaryOperator::LessThan(_) => false,
            BinaryOperator::LessThanOrEqual(_) => true,
            _ => return self.invalid_for(r#for, "use `<` or `<=` as the loop comparison"),
        };
        let end = self.lower_expression(condition.rhs)?;
        if !stable_range_bound(condition.rhs) {
            return self.invalid_for(
                r#for,
                "use a literal or top-level constant as the stable loop bound",
            );
        }

        let Some(increment) = exactly_one(r#for.increments.as_slice()) else {
            return self.invalid_for(r#for, "increment the loop variable exactly once");
        };
        let valid_increment = match increment {
            Expression::UnaryPostfix(increment) => {
                matches!(increment.operator, UnaryPostfixOperator::PostIncrement(_))
                    && is_direct_variable(increment.operand, &name)
            }
            Expression::UnaryPrefix(increment) => {
                matches!(increment.operator, UnaryPrefixOperator::PreIncrement(_))
                    && is_direct_variable(increment.operand, &name)
            }
            _ => false,
        };
        if !valid_increment {
            return self.invalid_for(r#for, "increment the loop variable with `$i++` or `++$i`");
        }

        let outer = self.declared.clone();
        self.declared.insert(name.clone());
        self.loop_depth += 1;
        let body = self.lower_statements(r#for.body.statements(), r#for.body.span());
        self.loop_depth -= 1;
        self.declared = outer;
        Some(Stmt::new(
            StmtKind::For {
                variable: Symbol::new(name),
                range: RangeExpr {
                    start,
                    end,
                    inclusive,
                    span: self.span(r#for.span()),
                },
                body,
            },
            self.span(r#for.span()),
        ))
    }

    fn lower_foreach(&mut self, foreach: &Foreach<'_>) -> Option<Vec<Stmt>> {
        if matches!(foreach.target, ForeachTarget::KeyValue(_)) {
            self.unsupported(
                foreach.target.span(),
                "key-value PHP foreach targets are outside Common Core",
                "iterate array values only, or use an indexed `for` loop",
            );
            return None;
        }
        let Expression::Variable(Variable::Direct(target)) = foreach.target.value() else {
            self.unsupported(
                foreach.target.span(),
                "foreach targets must be directly named value variables",
                "write `foreach ($array as $value)`",
            );
            return None;
        };
        let target_name = self.variable_name(target.name);
        if self.declared.contains(&target_name) {
            self.unsupported(
                foreach.target.span(),
                "foreach value variables must be fresh in Common Core",
                "use a new value variable for this loop",
            );
            return None;
        }

        let span = self.span(foreach.span());
        let collection_name = self.temporary("each");
        let collection = self.lower_expression(foreach.expression)?;
        self.declared.insert(collection_name.clone());
        let collection_var = Expr::new(
            ExprKind::Var(Symbol::new(collection_name.clone())),
            self.span(foreach.expression.span()),
        );
        let index_name = self.temporary("index");
        let index_var = Expr::new(ExprKind::Var(Symbol::new(index_name.clone())), span);
        let mut body_declared = self.declared.clone();
        body_declared.insert(index_name.clone());
        body_declared.insert(target_name.clone());
        let outer = std::mem::replace(&mut self.declared, body_declared);
        self.loop_depth += 1;
        let mut body = self.lower_statements(foreach.body.statements(), foreach.body.span());
        self.loop_depth -= 1;
        self.declared = outer;
        body.statements.insert(
            0,
            Stmt::new(
                StmtKind::Let {
                    name: Symbol::new(target_name),
                    ty: None,
                    init: Expr::new(
                        ExprKind::Index {
                            base: Box::new(collection_var.clone()),
                            index: Box::new(index_var),
                        },
                        span,
                    ),
                },
                span,
            ),
        );
        Some(vec![
            Stmt::new(
                StmtKind::Let {
                    name: Symbol::new(collection_name),
                    ty: None,
                    init: collection,
                },
                span,
            ),
            Stmt::new(
                StmtKind::For {
                    variable: Symbol::new(index_name),
                    range: RangeExpr {
                        start: Expr::new(ExprKind::Literal(Literal::Int(0)), span),
                        end: Expr::new(ExprKind::ArrayLength(Box::new(collection_var)), span),
                        inclusive: false,
                        span,
                    },
                    body,
                },
                span,
            ),
        ])
    }

    fn invalid_for<T>(&mut self, r#for: &For<'_>, requirement: &str) -> Option<T> {
        self.unsupported(
            r#for.span(),
            "this PHP for loop is outside the ascending Common Core range form",
            &format!("{requirement}; use `for ($i = start; $i < end; $i++)`"),
        );
        None
    }
}

fn exactly_one<T>(items: &[T]) -> Option<&T> {
    let [item] = items else {
        return None;
    };
    Some(item)
}

fn is_direct_variable(expression: &Expression<'_>, expected: &str) -> bool {
    matches!(
        expression,
        Expression::Variable(Variable::Direct(variable))
            if variable.name.strip_prefix(b"$") == Some(expected.as_bytes())
    )
}

fn stable_range_bound(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Literal(_) | Expression::ConstantAccess(_) => true,
        Expression::Parenthesized(parenthesized) => stable_range_bound(parenthesized.expression),
        Expression::UnaryPrefix(unary)
            if matches!(
                unary.operator,
                UnaryPrefixOperator::Negation(_) | UnaryPrefixOperator::Plus(_)
            ) =>
        {
            matches!(unary.operand, Expression::Literal(_))
        }
        _ => false,
    }
}

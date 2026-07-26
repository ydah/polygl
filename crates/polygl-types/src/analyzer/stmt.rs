use polygl_hir::{Block, Place, PlaceKind, RangeExpr, Stmt, StmtKind};

use crate::solver::{InferType, SolveError};

use super::{Analyzer, BodyContext};

impl Analyzer {
    pub(super) fn infer_block(
        &mut self,
        block: &mut Block,
        context: &mut BodyContext,
        nested: bool,
    ) {
        if nested {
            context.environment.push();
        }
        for statement in &mut block.statements {
            self.infer_statement(statement, context);
        }
        if nested {
            context.environment.pop();
        }
    }

    fn infer_statement(&mut self, statement: &mut Stmt, context: &mut BodyContext) {
        let statement_key = statement as *mut Stmt as usize;
        let span = statement.span;
        match &mut statement.kind {
            StmtKind::Let { name, ty, init } => {
                let binding = self.solver.fresh();
                let annotation = ty.as_ref().map(Self::annotated_type);
                if annotation.is_some() {
                    self.annotated_bindings.insert(statement_key);
                    self.solver.mark_fixed(&binding);
                }
                if let Some(expected) = &annotation
                    && let Err(error) = self.solver.assign(binding.clone(), expected.clone())
                {
                    self.solve_error(error, span, "E0303");
                }
                let actual = self.infer_expr(init, context);
                self.reject_unit_value(&actual, init.span);
                let result = if let Some(expected) = annotation {
                    self.solver.assign(expected, actual)
                } else {
                    self.solver.join(binding.clone(), actual)
                };
                if let Err(error) = result {
                    self.solve_error(error, span, "E0303");
                }
                context.environment.insert(name, binding.clone());
                self.binding_types.insert(statement_key, binding);
            }
            StmtKind::Assign { target, value } => {
                let expected = self.infer_place(target, context);
                let actual = self.infer_expr(value, context);
                self.reject_unit_value(&actual, value.span);
                let result = match &target.kind {
                    PlaceKind::Var(_) => self.solver.reassign(expected, actual),
                    PlaceKind::Index { .. } | PlaceKind::Field { .. } => {
                        self.solver.assign(expected, actual)
                    }
                };
                if let Err(error) = result {
                    self.reassignment_error(error, span);
                }
            }
            StmtKind::Expr(expression) => {
                self.infer_expr(expression, context);
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition_type = self.infer_expr(condition, context);
                self.expect_condition(condition_type, condition.span);
                self.infer_block(then_block, context, true);
                if let Some(else_block) = else_block {
                    self.infer_block(else_block, context, true);
                }
            }
            StmtKind::While { condition, body } => {
                let condition_type = self.infer_expr(condition, context);
                self.expect_condition(condition_type, condition.span);
                context.loop_depth += 1;
                self.infer_block(body, context, true);
                context.loop_depth -= 1;
            }
            StmtKind::For {
                variable,
                range,
                body,
            } => {
                self.infer_range(range, context);
                context.environment.push();
                context.environment.insert(variable, InferType::Int);
                context.loop_depth += 1;
                self.infer_block(body, context, false);
                context.loop_depth -= 1;
                context.environment.pop();
            }
            StmtKind::Return(value) => {
                let returned = value.as_mut().map_or(InferType::Unit, |expression| {
                    self.infer_expr(expression, context)
                });
                context.returns.push(returned);
            }
            StmtKind::Break => {
                if context.loop_depth == 0 {
                    self.loop_control_error("break", span);
                }
            }
            StmtKind::Continue => {
                if context.loop_depth == 0 {
                    self.loop_control_error("continue", span);
                }
            }
        }
    }

    fn infer_range(&mut self, range: &mut RangeExpr, context: &mut BodyContext) {
        let start = self.infer_expr(&mut range.start, context);
        self.expect_integer(start, range.start.span);
        let end = self.infer_expr(&mut range.end, context);
        self.expect_integer(end, range.end.span);
    }

    fn infer_place(&mut self, place: &mut Place, context: &mut BodyContext) -> InferType {
        if let Some(name) = assignment_root(place)
            && context.environment.is_mutable(name) == Some(false)
        {
            self.constant_assignment_error(name.as_str(), place.span);
            return InferType::Error;
        }
        match &mut place.kind {
            PlaceKind::Var(name) => context.environment.get(name).unwrap_or_else(|| {
                self.unknown_variable_error(name.as_str(), place.span);
                InferType::Error
            }),
            PlaceKind::Index { base, index } => {
                let inferred_base = self.infer_expr(base, context);
                let base_type = self.solver.resolve(&inferred_base);
                let index = self.infer_expr(index, context);
                match base_type {
                    InferType::Array(element) => {
                        self.expect_integer(index, place.span);
                        *element
                    }
                    InferType::Map(value) => {
                        if let Err(error) = self.solver.assign(InferType::Str, index) {
                            self.solve_error(error, place.span, "E0303");
                        }
                        *value
                    }
                    InferType::Error => InferType::Error,
                    actual => {
                        let element = self.solver.fresh();
                        self.solve_error(
                            SolveError::Mismatch {
                                expected: InferType::Array(Box::new(element)),
                                actual,
                            },
                            place.span,
                            "E0303",
                        );
                        InferType::Error
                    }
                }
            }
            PlaceKind::Field { base, field } => {
                let inferred_base = self.infer_expr(base, context);
                let base = self.solver.resolve(&inferred_base);
                self.field_type(base, field.as_str(), place.span)
            }
        }
    }

    fn expect_condition(&mut self, actual: InferType, span: polygl_span::Span) {
        if let Err(error) = self.solver.assign(InferType::Bool, actual) {
            if let SolveError::Mismatch { actual, .. } = error {
                self.condition_error(&actual, span);
            } else {
                self.solve_error(error, span, "E0301");
            }
        }
    }

    fn expect_integer(&mut self, actual: InferType, span: polygl_span::Span) {
        if let Err(error) = self.solver.assign(InferType::Int, actual) {
            self.solve_error(error, span, "E0303");
        }
    }
}

pub(super) fn assignment_root(place: &Place) -> Option<&polygl_hir::Symbol> {
    match &place.kind {
        PlaceKind::Var(name) => Some(name),
        PlaceKind::Index { base, .. } | PlaceKind::Field { base, .. } => expression_root(base),
    }
}

fn expression_root(expression: &polygl_hir::Expr) -> Option<&polygl_hir::Symbol> {
    match &expression.kind {
        polygl_hir::ExprKind::Var(name) => Some(name),
        polygl_hir::ExprKind::Index { base, .. } | polygl_hir::ExprKind::Field { base, .. } => {
            expression_root(base)
        }
        polygl_hir::ExprKind::Literal(_)
        | polygl_hir::ExprKind::Uniform { .. }
        | polygl_hir::ExprKind::Binary { .. }
        | polygl_hir::ExprKind::Unary { .. }
        | polygl_hir::ExprKind::Call { .. }
        | polygl_hir::ExprKind::ArrayLength(_)
        | polygl_hir::ExprKind::Array(_)
        | polygl_hir::ExprKind::Map(_)
        | polygl_hir::ExprKind::Struct { .. }
        | polygl_hir::ExprKind::Vector { .. }
        | polygl_hir::ExprKind::NilCheck(_)
        | polygl_hir::ExprKind::FalsyCheck(_) => None,
    }
}

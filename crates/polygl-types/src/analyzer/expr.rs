use std::collections::HashSet;

use polygl_builtins::{BuiltinTable, BuiltinType, BuiltinValueType};
use polygl_hir::{BinOp, Callee, Expr, ExprKind, Literal, UnOp};

use crate::solver::{InferType, SolveError};

use super::{Analyzer, BodyContext};

impl Analyzer {
    pub(super) fn infer_expr(
        &mut self,
        expression: &mut Expr,
        context: &mut BodyContext,
    ) -> InferType {
        let expression_key = expression as *mut Expr as usize;
        let span = expression.span;
        let inferred = match &mut expression.kind {
            ExprKind::Literal(literal) => self.literal_type(literal),
            ExprKind::Var(name) => context.environment.get(name).unwrap_or_else(|| {
                self.unknown_variable_error(name.as_str(), span);
                InferType::Error
            }),
            ExprKind::Binary { op, left, right } => {
                let left = self.infer_expr(left, context);
                let right = self.infer_expr(right, context);
                self.infer_binary(*op, left, right, span)
            }
            ExprKind::Unary { op, operand } => {
                let operand = self.infer_expr(operand, context);
                self.infer_unary(*op, operand, span)
            }
            ExprKind::Call { callee, args } => {
                let argument_types = args
                    .iter_mut()
                    .map(|argument| self.infer_expr(argument, context))
                    .collect::<Vec<_>>();
                self.infer_call(callee, &argument_types, span, expression_key)
            }
            ExprKind::Index { base, index } => {
                let inferred_base = self.infer_expr(base, context);
                let base = self.solver.resolve(&inferred_base);
                let index = self.infer_expr(index, context);
                match base {
                    InferType::Array(element) => {
                        self.expect(InferType::Int, index, span);
                        *element
                    }
                    InferType::Map(value) => {
                        self.expect(InferType::Str, index, span);
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
                            span,
                            "E0303",
                        );
                        InferType::Error
                    }
                }
            }
            ExprKind::Field { base, field } => {
                let inferred_base = self.infer_expr(base, context);
                let base = self.solver.resolve(&inferred_base);
                self.field_type(base, field.as_str(), span)
            }
            ExprKind::Array(items) => {
                let mut element = self.solver.fresh();
                for item in items {
                    let item = self.infer_expr(item, context);
                    self.reject_unit_value(&item, span);
                    element = self.join_or_error(element, item, span, "E0303");
                }
                InferType::Array(Box::new(element))
            }
            ExprKind::Map(entries) => {
                let mut value = self.solver.fresh();
                for entry in entries {
                    let key = self.infer_expr(&mut entry.key, context);
                    self.expect(InferType::Str, key, entry.key.span);
                    let entry_value = self.infer_expr(&mut entry.value, context);
                    self.reject_unit_value(&entry_value, entry.value.span);
                    value = self.join_or_error(value, entry_value, entry.value.span, "E0303");
                }
                InferType::Map(Box::new(value))
            }
            ExprKind::Struct { name, fields } => {
                self.infer_struct(name.as_str(), fields, context, span)
            }
            ExprKind::Vector { size, args } => {
                if !(2..=4).contains(size) {
                    self.invalid_dimension_error("vector", *size, span);
                } else if args.len() != usize::from(*size) {
                    self.arity_error(&format!("vec{size}"), usize::from(*size), args.len(), span);
                }
                for argument in args {
                    let actual = self.infer_expr(argument, context);
                    self.expect(InferType::Float, actual, argument.span);
                }
                InferType::Vector(*size)
            }
            ExprKind::NilCheck(value) => {
                let inferred = self.infer_expr(value, context);
                match self.solver.resolve(&inferred) {
                    InferType::Option(_) | InferType::Error => {}
                    InferType::Var(_) => {
                        let inner = self.solver.fresh();
                        if let Err(error) = self
                            .solver
                            .equal(inferred, InferType::Option(Box::new(inner)))
                        {
                            self.solve_error(error, span, "E0303");
                        }
                    }
                    actual => {
                        let inner = self.solver.fresh();
                        self.solve_error(
                            SolveError::Mismatch {
                                expected: InferType::Option(Box::new(inner)),
                                actual,
                            },
                            span,
                            "E0303",
                        );
                    }
                }
                InferType::Bool
            }
            ExprKind::FalsyCheck(value) => {
                let inferred = self.infer_expr(value, context);
                self.reject_unit_value(&inferred, value.span);
                InferType::Bool
            }
        };
        self.expression_types
            .insert(expression_key, inferred.clone());
        inferred
    }

    fn literal_type(&mut self, literal: &Literal) -> InferType {
        match literal {
            Literal::Int(_) => InferType::Int,
            Literal::Float(_) => InferType::Float,
            Literal::Bool(_) => InferType::Bool,
            Literal::Str(_) => InferType::Str,
            Literal::None => InferType::Option(Box::new(self.solver.fresh())),
        }
    }

    fn infer_binary(
        &mut self,
        operator: BinOp,
        left: InferType,
        right: InferType,
        span: polygl_span::Span,
    ) -> InferType {
        match operator {
            BinOp::Add => {
                let resolved_left = self.solver.resolve(&left);
                let resolved_right = self.solver.resolve(&right);
                match (&resolved_left, &resolved_right) {
                    (InferType::Str, InferType::Str) => InferType::Str,
                    (InferType::Str, InferType::Var(_)) => {
                        self.expect_same(InferType::Str, right, span);
                        InferType::Str
                    }
                    (InferType::Var(_), InferType::Str) => {
                        self.expect_same(left, InferType::Str, span);
                        InferType::Str
                    }
                    (InferType::Str, _) | (_, InferType::Str) => {
                        self.solve_error(
                            SolveError::Mismatch {
                                expected: resolved_left,
                                actual: resolved_right,
                            },
                            span,
                            "E0303",
                        );
                        InferType::Error
                    }
                    (InferType::Var(_), InferType::Var(_)) => {
                        let result = self.solver.fresh();
                        self.defer_add(left, right, result.clone(), span);
                        result
                    }
                    _ => self.numeric_result(left, right, span, false),
                }
            }
            BinOp::Sub | BinOp::Mul | BinOp::Rem => self.numeric_result(left, right, span, false),
            BinOp::DivInt => self.numeric_result(left, right, span, false),
            BinOp::DivFloat => self.numeric_result(left, right, span, true),
            BinOp::Eq | BinOp::NotEq => {
                self.expect_same(left, right, span);
                InferType::Bool
            }
            BinOp::Less | BinOp::LessEq | BinOp::Greater | BinOp::GreaterEq => {
                self.expect_ordered_same(left, right, span);
                InferType::Bool
            }
            BinOp::And | BinOp::Or => {
                self.expect(InferType::Bool, left, span);
                self.expect(InferType::Bool, right, span);
                InferType::Bool
            }
            BinOp::StrConcat => {
                self.expect(InferType::Str, left, span);
                self.expect(InferType::Str, right, span);
                InferType::Str
            }
        }
    }

    fn infer_unary(
        &mut self,
        operator: UnOp,
        operand: InferType,
        span: polygl_span::Span,
    ) -> InferType {
        match operator {
            UnOp::Neg => {
                if self.solver.require_numeric(&operand).is_ok() {
                    operand
                } else {
                    let actual = self.solver.resolve(&operand);
                    self.solve_error(
                        SolveError::Mismatch {
                            expected: InferType::Float,
                            actual,
                        },
                        span,
                        "E0303",
                    );
                    InferType::Error
                }
            }
            UnOp::Not => {
                self.expect(InferType::Bool, operand, span);
                InferType::Bool
            }
        }
    }

    fn infer_call(
        &mut self,
        callee: &mut Callee,
        arguments: &[InferType],
        span: polygl_span::Span,
        expression_key: usize,
    ) -> InferType {
        if arguments
            .iter()
            .any(|argument| self.solver.resolve(argument) == InferType::Unit)
        {
            self.unit_value_error(span);
            return InferType::Error;
        }
        match callee {
            Callee::Builtin(id) => {
                let Some(builtin) = BuiltinTable::all().iter().find(|builtin| builtin.id == *id)
                else {
                    self.unknown_function_error(&format!("builtin#{}", id.raw()), span);
                    return InferType::Error;
                };
                let required = builtin
                    .signature
                    .params
                    .iter()
                    .filter(|parameter| parameter.default.is_none())
                    .count();
                if arguments.len() < required || arguments.len() > builtin.signature.params.len() {
                    if required == builtin.signature.params.len() {
                        self.arity_error(builtin.name, required, arguments.len(), span);
                    } else {
                        self.arity_range_error(
                            builtin.name,
                            required,
                            builtin.signature.params.len(),
                            arguments.len(),
                            span,
                        );
                    }
                    return InferType::Error;
                }
                for (parameter, actual) in builtin.signature.params.iter().zip(arguments) {
                    let expected = builtin_type(parameter.ty);
                    self.expect(expected, actual.clone(), span);
                }
                builtin_type(builtin.signature.result)
            }
            Callee::User(name) => {
                let source_name = name.clone();
                self.infer_user_call(&source_name, arguments, span, expression_key)
            }
        }
    }

    fn numeric_result(
        &mut self,
        left: InferType,
        right: InferType,
        span: polygl_span::Span,
        force_float: bool,
    ) -> InferType {
        if let Err(error) = self.solver.require_numeric(&left) {
            self.solve_error(error, span, "E0303");
            return InferType::Error;
        }
        if let Err(error) = self.solver.require_numeric(&right) {
            self.solve_error(error, span, "E0303");
            return InferType::Error;
        }
        if force_float {
            self.expect(InferType::Float, left, span);
            self.expect(InferType::Float, right, span);
            return InferType::Float;
        }
        match self.solver.join(left, right) {
            Ok(result) => result,
            Err(error) => {
                self.solve_error(error, span, "E0303");
                InferType::Error
            }
        }
    }

    fn expect(&mut self, expected: InferType, actual: InferType, span: polygl_span::Span) {
        if let Err(error) = self.solver.assign(expected, actual) {
            self.solve_error(error, span, "E0303");
        }
    }

    pub(super) fn reject_unit_value(&mut self, ty: &InferType, span: polygl_span::Span) {
        if self.solver.resolve(ty) == InferType::Unit {
            self.unit_value_error(span);
        }
    }

    fn expect_same(&mut self, left: InferType, right: InferType, span: polygl_span::Span) {
        if let Err(error) = self.solver.equal(left, right) {
            self.solve_error(error, span, "E0303");
        }
    }

    fn expect_ordered_same(&mut self, left: InferType, right: InferType, span: polygl_span::Span) {
        if let Err(error) = self.solver.require_numeric(&left) {
            self.solve_error(error, span, "E0303");
            return;
        }
        if let Err(error) = self.solver.require_numeric(&right) {
            self.solve_error(error, span, "E0303");
            return;
        }
        if let Err(error) = self.solver.equal(left, right) {
            self.solve_error(error, span, "E0303");
        }
    }

    fn join_or_error(
        &mut self,
        left: InferType,
        right: InferType,
        span: polygl_span::Span,
        code: &str,
    ) -> InferType {
        match self.solver.join(left, right) {
            Ok(ty) => ty,
            Err(error) => {
                self.solve_error(error, span, code);
                InferType::Error
            }
        }
    }

    pub(super) fn field_type(
        &mut self,
        base: InferType,
        field: &str,
        span: polygl_span::Span,
    ) -> InferType {
        let InferType::Struct(name) = base else {
            if base != InferType::Error {
                self.solve_error(
                    SolveError::Mismatch {
                        expected: InferType::Struct(polygl_hir::Symbol::new("Struct")),
                        actual: base,
                    },
                    span,
                    "E0303",
                );
            }
            return InferType::Error;
        };
        let result = self
            .structs
            .get(name.as_str())
            .and_then(|definition| {
                definition
                    .fields
                    .iter()
                    .find(|definition| definition.name.as_str() == field)
            })
            .and_then(|definition| definition.ty.as_ref())
            .map(Self::annotated_type)
            .or_else(|| {
                BuiltinTable::find_struct(name.as_str())
                    .and_then(|definition| {
                        definition
                            .fields
                            .iter()
                            .find(|definition| definition.name == field)
                    })
                    .map(|definition| builtin_value_type(definition.ty))
            });
        if let Some(result) = result {
            result
        } else {
            let unknown = self.solver.fresh();
            self.unresolved_error(&unknown, span, Some(&format!("{name}.{field}")));
            InferType::Error
        }
    }

    fn infer_struct(
        &mut self,
        name: &str,
        fields: &mut [polygl_hir::FieldInit],
        context: &mut BodyContext,
        span: polygl_span::Span,
    ) -> InferType {
        let Some(definition) = self.structs.get(name).cloned() else {
            self.unknown_function_error(name, span);
            return InferType::Error;
        };
        let mut initialized = HashSet::new();
        for field in fields {
            let actual = self.infer_expr(&mut field.value, context);
            self.reject_unit_value(&actual, field.value.span);
            if !initialized.insert(field.name.as_str().to_owned()) {
                self.duplicate_struct_field_error(field.name.as_str(), field.span);
                continue;
            }
            let declared = definition
                .fields
                .iter()
                .find(|definition| definition.name == field.name);
            if let Some(expected) = declared
                .and_then(|definition| definition.ty.as_ref())
                .map(Self::annotated_type)
            {
                self.expect(expected, actual, field.span);
            } else {
                self.unknown_struct_field_error(name, field.name.as_str(), field.span);
            }
        }
        for field in &definition.fields {
            if !initialized.contains(field.name.as_str()) {
                self.missing_struct_field_error(name, field.name.as_str(), span);
            }
        }
        InferType::Struct(definition.name)
    }
}

pub(super) fn builtin_type(ty: BuiltinType) -> InferType {
    match ty {
        BuiltinType::Void => InferType::Unit,
        BuiltinType::Int => InferType::Int,
        BuiltinType::Float => InferType::Float,
        BuiltinType::Bool => InferType::Bool,
        BuiltinType::Str => InferType::Str,
    }
}

pub(super) fn builtin_value_type(ty: BuiltinValueType) -> InferType {
    match ty {
        BuiltinValueType::Scalar(ty) => builtin_type(ty),
        BuiltinValueType::Option(ty) => InferType::Option(Box::new(builtin_type(ty))),
    }
}

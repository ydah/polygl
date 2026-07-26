use polygl_builtins::BuiltinTable;
use polygl_hir::{
    BinOp, Block, Callee, Expr, ExprKind, Literal, Place, PlaceKind, Stmt, StmtKind, UnOp,
};

use crate::Type;
use crate::solver::InferType;

use super::expr::{builtin_type, builtin_value_type};
use super::stmt::assignment_root;
use super::{Analyzer, BodyContext, Environment};

impl Analyzer {
    pub(super) fn annotate_block(
        &mut self,
        block: &mut Block,
        inferred: &BodyContext,
        nested: bool,
    ) {
        let mut environment = inferred.environment.clone();
        self.annotate_block_with_environment(block, inferred, &mut environment, nested);
    }

    fn annotate_block_with_environment(
        &mut self,
        block: &mut Block,
        inferred: &BodyContext,
        environment: &mut Environment,
        nested: bool,
    ) {
        if nested {
            environment.push();
        }
        for statement in &mut block.statements {
            self.annotate_statement(statement, inferred, environment);
        }
        if nested {
            environment.pop();
        }
    }

    fn annotate_statement(
        &mut self,
        statement: &mut Stmt,
        inferred: &BodyContext,
        environment: &mut Environment,
    ) {
        let statement_key = statement as *mut Stmt as usize;
        match &mut statement.kind {
            StmtKind::Let { name, ty, init } => {
                let init_type = self.annotate_expr(init, environment);
                self.validate_final_value(&init_type, init.span);
                if let Some(binding) = self.binding_types.get(&statement_key).cloned() {
                    let init_type = InferType::from_type(&init_type);
                    let refreshed = if self.annotated_bindings.contains(&statement_key) {
                        let expected = ty
                            .as_ref()
                            .map(Self::annotated_type)
                            .expect("annotated bindings retain their source type");
                        self.solver.assign(expected, init_type)
                    } else {
                        self.solver.join(binding.clone(), init_type)
                    };
                    if let Err(error) = refreshed {
                        self.solve_error(error, statement.span, "E0303");
                    }
                }
                if let Some(binding) = self.binding_types.get(&statement_key).cloned()
                    && let Some(resolved) =
                        self.resolve_expression_type(&binding, statement.span, Some(name.as_str()))
                {
                    *ty = Some(resolved.to_expr(statement.span));
                    environment.insert(name, binding);
                }
            }
            StmtKind::Assign { target, value } => {
                if let Some(name) = assignment_root(target)
                    && environment.is_mutable(name) == Some(false)
                {
                    self.constant_assignment_error(name.as_str(), target.span);
                    return;
                }
                let target_type = self.annotate_place(target, environment);
                let value_type = self.annotate_expr(value, environment);
                self.validate_final_value(&value_type, value.span);
                let variable_binding = match &target.kind {
                    PlaceKind::Var(name) => environment.get(name),
                    PlaceKind::Index { .. } | PlaceKind::Field { .. } => None,
                };
                let result = match &target.kind {
                    PlaceKind::Var(_) => variable_binding.clone().map_or_else(
                        || {
                            Err(crate::solver::SolveError::Mismatch {
                                expected: InferType::from_type(&target_type),
                                actual: InferType::from_type(&value_type),
                            })
                        },
                        |binding| {
                            self.solver
                                .reassign(binding, InferType::from_type(&value_type))
                        },
                    ),
                    PlaceKind::Index { .. } | PlaceKind::Field { .. } => self.solver.assign(
                        InferType::from_type(&target_type),
                        InferType::from_type(&value_type),
                    ),
                };
                match result {
                    Ok(_) => {
                        if let PlaceKind::Var(name) = &target.kind
                            && let Some(binding) = variable_binding
                        {
                            environment.insert(name, binding);
                        }
                    }
                    Err(error) => self.reassignment_error(error, statement.span),
                }
            }
            StmtKind::Expr(expression) => {
                self.annotate_expr(expression, environment);
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let condition_type = self.annotate_expr(condition, environment);
                self.validate_final_condition(&condition_type, condition.span);
                self.annotate_block_with_environment(then_block, inferred, environment, true);
                if let Some(else_block) = else_block {
                    self.annotate_block_with_environment(else_block, inferred, environment, true);
                }
            }
            StmtKind::While { condition, body } => {
                let condition_type = self.annotate_expr(condition, environment);
                self.validate_final_condition(&condition_type, condition.span);
                self.annotate_block_with_environment(body, inferred, environment, true);
            }
            StmtKind::For {
                variable,
                range,
                body,
            } => {
                let start = self.annotate_expr(&mut range.start, environment);
                let end = self.annotate_expr(&mut range.end, environment);
                self.validate_final_integer(&start, range.start.span);
                self.validate_final_integer(&end, range.end.span);
                environment.push();
                environment.insert(variable, InferType::Int);
                self.annotate_block_with_environment(body, inferred, environment, false);
                environment.pop();
            }
            StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.annotate_expr(value, environment);
                }
            }
            StmtKind::Break | StmtKind::Continue => {}
        }
    }

    pub(super) fn annotate_expr(
        &mut self,
        expression: &mut Expr,
        environment: &Environment,
    ) -> Type {
        let expression_key = expression as *mut Expr as usize;
        let span = expression.span;
        let needs_context = matches!(&expression.kind, ExprKind::Literal(Literal::None))
            || matches!(&expression.kind, ExprKind::Array(items) if items.is_empty())
            || matches!(&expression.kind, ExprKind::Map(entries) if entries.is_empty());
        let structural_type = match &mut expression.kind {
            ExprKind::Literal(literal) => match literal {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::Str(_) => Type::Str,
                Literal::None => Type::Option(Box::new(Type::Unit)),
            },
            ExprKind::Var(name) => environment
                .get(name)
                .and_then(|ty| self.resolve_expression_type(&ty, span, Some(name.as_str())))
                .unwrap_or(Type::Unit),
            ExprKind::Uniform { declared, .. } => Type::from_expr(declared),
            ExprKind::Binary { op, left, right } => {
                let left = self.annotate_expr(left, environment);
                let right = self.annotate_expr(right, environment);
                let source_operator = *op;
                let result = annotated_binary(op, &left, &right);
                self.validate_final_binary(source_operator, &left, &right, span);
                result
            }
            ExprKind::Unary { op, operand } => {
                let operand = self.annotate_expr(operand, environment);
                match op {
                    UnOp::Neg => {
                        if !matches!(operand, Type::Int | Type::Float) {
                            self.solve_error(
                                crate::solver::SolveError::Mismatch {
                                    expected: InferType::Float,
                                    actual: InferType::from_type(&operand),
                                },
                                span,
                                "E0303",
                            );
                        }
                        operand
                    }
                    UnOp::Not => {
                        self.validate_final_condition(&operand, span);
                        Type::Bool
                    }
                }
            }
            ExprKind::Call { callee, args } => {
                let supplied_arguments = args
                    .iter_mut()
                    .map(|argument| self.annotate_expr(argument, environment))
                    .collect::<Vec<_>>();
                for (argument, ty) in args.iter().zip(&supplied_arguments) {
                    self.validate_final_value(ty, argument.span);
                }
                match callee {
                    Callee::Builtin(id) => {
                        self.validate_final_builtin(*id, &supplied_arguments, span);
                        self.callee_result(callee).unwrap_or(Type::Unit)
                    }
                    Callee::User(name) => {
                        if self.pending_instances.contains_key(&expression_key) {
                            let Some(instance) =
                                self.finish_instance(expression_key, &supplied_arguments)
                            else {
                                return Type::Unit;
                            };
                            *name = polygl_hir::Symbol::new(instance.name);
                            instance.result
                        } else {
                            self.instance_returns
                                .get(name.as_str())
                                .cloned()
                                .unwrap_or(Type::Unit)
                        }
                    }
                    Callee::Method(_) => {
                        unreachable!("inference resolves method calls before annotation")
                    }
                }
            }
            ExprKind::Index { base, index } => {
                let base = self.annotate_expr(base, environment);
                let index_type = self.annotate_expr(index, environment);
                match base {
                    Type::Array(element) => {
                        self.validate_final_integer(&index_type, index.span);
                        *element
                    }
                    Type::Map(element) => {
                        if index_type != Type::Str {
                            self.solve_error(
                                crate::solver::SolveError::Mismatch {
                                    expected: InferType::Str,
                                    actual: InferType::from_type(&index_type),
                                },
                                index.span,
                                "E0303",
                            );
                        }
                        *element
                    }
                    _ => Type::Unit,
                }
            }
            ExprKind::Field { base, field } => {
                let base = self.annotate_expr(base, environment);
                self.annotated_field_type(&base, field.as_str())
                    .unwrap_or(Type::Unit)
            }
            ExprKind::ArrayLength(value) => {
                let value_type = self.annotate_expr(value, environment);
                if !matches!(value_type, Type::Array(_)) {
                    let element = self.solver.fresh();
                    self.solve_error(
                        crate::solver::SolveError::Mismatch {
                            expected: InferType::Array(Box::new(element)),
                            actual: InferType::from_type(&value_type),
                        },
                        span,
                        "E0303",
                    );
                }
                Type::Int
            }
            ExprKind::Array(items) => {
                let mut element = None;
                for item in items {
                    let item_type = self.annotate_expr(item, environment);
                    self.validate_final_value(&item_type, item.span);
                    element = Some(
                        element.map_or(item_type.clone(), |current| join_types(current, item_type)),
                    );
                }
                let element = element.unwrap_or(Type::Unit);
                Type::Array(Box::new(element))
            }
            ExprKind::Map(entries) => {
                let value = entries
                    .iter_mut()
                    .map(|entry| {
                        let key = self.annotate_expr(&mut entry.key, environment);
                        if key != Type::Str {
                            self.solve_error(
                                crate::solver::SolveError::Mismatch {
                                    expected: InferType::Str,
                                    actual: InferType::from_type(&key),
                                },
                                entry.key.span,
                                "E0303",
                            );
                        }
                        let value = self.annotate_expr(&mut entry.value, environment);
                        self.validate_final_value(&value, entry.value.span);
                        value
                    })
                    .reduce(join_types)
                    .unwrap_or(Type::Unit);
                Type::Map(Box::new(value))
            }
            ExprKind::Struct { name, fields } => {
                let definition = self.structs.get(name.as_str()).cloned();
                for field in fields {
                    let actual = self.annotate_expr(&mut field.value, environment);
                    self.validate_final_value(&actual, field.value.span);
                    if let Some(expected) = definition
                        .as_ref()
                        .and_then(|definition| {
                            definition
                                .fields
                                .iter()
                                .find(|definition| definition.name == field.name)
                        })
                        .and_then(|definition| definition.ty.as_ref())
                        .map(Type::from_expr)
                        && let Err(error) = self.solver.assign(
                            InferType::from_type(&expected),
                            InferType::from_type(&actual),
                        )
                    {
                        self.solve_error(error, field.span, "E0303");
                    }
                }
                Type::Struct(name.clone())
            }
            ExprKind::Vector { size, args } => {
                if !(2..=4).contains(size) {
                    self.invalid_dimension_error("vector", *size, span);
                }
                let mut components = 0_usize;
                for argument in args {
                    let actual = self.annotate_expr(argument, environment);
                    match actual {
                        Type::Vector(argument_size) => {
                            components += usize::from(argument_size);
                        }
                        Type::Int | Type::Float => {
                            components += 1;
                            if let Err(error) = self
                                .solver
                                .assign(InferType::Float, InferType::from_type(&actual))
                            {
                                self.solve_error(error, argument.span, "E0303");
                            }
                        }
                        _ => self.solve_error(
                            crate::solver::SolveError::Mismatch {
                                expected: InferType::Float,
                                actual: InferType::from_type(&actual),
                            },
                            argument.span,
                            "E0303",
                        ),
                    }
                }
                if components != usize::from(*size) {
                    self.arity_error(
                        &format!("vec{size} components"),
                        usize::from(*size),
                        components,
                        span,
                    );
                }
                Type::Vector(*size)
            }
            ExprKind::NilCheck(value) => {
                let actual = self.annotate_expr(value, environment);
                if !matches!(actual, Type::Option(_)) {
                    let inner = self.solver.fresh();
                    self.solve_error(
                        crate::solver::SolveError::Mismatch {
                            expected: InferType::Option(Box::new(inner)),
                            actual: InferType::from_type(&actual),
                        },
                        span,
                        "E0303",
                    );
                }
                Type::Bool
            }
            ExprKind::FalsyCheck(value) => {
                let actual = self.annotate_expr(value, environment);
                self.validate_final_value(&actual, value.span);
                Type::Bool
            }
        };
        let ty = if needs_context {
            self.expression_types
                .get(&expression_key)
                .cloned()
                .and_then(|inferred| self.resolve_expression_type(&inferred, span, None))
                .unwrap_or(structural_type)
        } else {
            structural_type
        };
        expression.ty = Some(ty.to_expr(span));
        ty
    }

    fn validate_final_binary(
        &mut self,
        operator: BinOp,
        left: &Type,
        right: &Type,
        span: polygl_span::Span,
    ) {
        let exact_comparison = matches!(
            operator,
            BinOp::Eq
                | BinOp::NotEq
                | BinOp::Less
                | BinOp::LessEq
                | BinOp::Greater
                | BinOp::GreaterEq
        );
        let valid_add = operator != BinOp::Add
            || (left == &Type::Str && right == &Type::Str)
            || (matches!(left, Type::Int | Type::Float)
                && matches!(right, Type::Int | Type::Float));
        let valid_multiply = operator != BinOp::Mul
            || matches!(
                (left, right),
                (Type::Int | Type::Float, Type::Int | Type::Float)
            )
            || matches!(
                (left, right),
                (Type::Matrix(left), Type::Matrix(right)) if left == right
            )
            || matches!(
                (left, right),
                (Type::Matrix(matrix), Type::Vector(vector)) if matrix == vector
            )
            || matches!(
                (left, right),
                (Type::Vector(_), Type::Int | Type::Float)
                    | (Type::Int | Type::Float, Type::Vector(_))
            );
        if (exact_comparison && left != right) || !valid_add || !valid_multiply {
            self.solve_error(
                crate::solver::SolveError::Mismatch {
                    expected: InferType::from_type(left),
                    actual: InferType::from_type(right),
                },
                span,
                "E0303",
            );
        }
    }

    fn validate_final_condition(&mut self, actual: &Type, span: polygl_span::Span) {
        if actual != &Type::Bool {
            self.condition_error(&InferType::from_type(actual), span);
        }
    }

    pub(super) fn validate_final_value(&mut self, actual: &Type, span: polygl_span::Span) {
        if !actual.is_value_type() {
            self.unit_value_error(span);
        }
    }

    fn validate_final_integer(&mut self, actual: &Type, span: polygl_span::Span) {
        if actual != &Type::Int {
            self.solve_error(
                crate::solver::SolveError::Mismatch {
                    expected: InferType::Int,
                    actual: InferType::from_type(actual),
                },
                span,
                "E0303",
            );
        }
    }

    fn validate_final_builtin(
        &mut self,
        id: polygl_hir::BuiltinId,
        arguments: &[Type],
        span: polygl_span::Span,
    ) {
        let Some(builtin) = BuiltinTable::all().iter().find(|builtin| builtin.id == id) else {
            return;
        };
        for (parameter, actual) in builtin.signature.params.iter().zip(arguments) {
            if let Err(error) = self
                .solver
                .assign(builtin_type(parameter.ty), InferType::from_type(actual))
            {
                self.solve_error(error, span, "E0303");
            }
        }
    }

    fn annotate_place(&mut self, place: &mut Place, environment: &Environment) -> Type {
        match &mut place.kind {
            PlaceKind::Var(name) => environment
                .get(name)
                .and_then(|ty| self.resolve_expression_type(&ty, place.span, Some(name.as_str())))
                .unwrap_or(Type::Unit),
            PlaceKind::Index { base, index } => {
                let base = self.annotate_expr(base, environment);
                let index_type = self.annotate_expr(index, environment);
                match base {
                    Type::Array(element) => {
                        self.validate_final_integer(&index_type, index.span);
                        *element
                    }
                    Type::Map(element) => {
                        if index_type != Type::Str {
                            self.solve_error(
                                crate::solver::SolveError::Mismatch {
                                    expected: InferType::Str,
                                    actual: InferType::from_type(&index_type),
                                },
                                index.span,
                                "E0303",
                            );
                        }
                        *element
                    }
                    _ => Type::Unit,
                }
            }
            PlaceKind::Field { base, field } => {
                let base = self.annotate_expr(base, environment);
                self.annotated_field_type(&base, field.as_str())
                    .unwrap_or(Type::Unit)
            }
        }
    }

    fn callee_result(&self, callee: &Callee) -> Option<Type> {
        match callee {
            Callee::Builtin(id) => BuiltinTable::all()
                .iter()
                .find(|builtin| builtin.id == *id)
                .map(|builtin| infer_to_type(builtin_type(builtin.signature.result))),
            Callee::User(name) => self.instance_returns.get(name.as_str()).cloned(),
            Callee::Method(_) => None,
        }
    }

    fn annotated_field_type(&self, base: &Type, field: &str) -> Option<Type> {
        let Type::Struct(name) = base else {
            return None;
        };
        self.structs
            .get(name.as_str())
            .and_then(|definition| {
                definition
                    .fields
                    .iter()
                    .find(|definition| definition.name.as_str() == field)
            })
            .and_then(|definition| definition.ty.as_ref())
            .map(Type::from_expr)
            .or_else(|| {
                BuiltinTable::find_struct(name.as_str())
                    .and_then(|definition| {
                        definition
                            .fields
                            .iter()
                            .find(|definition| definition.name == field)
                    })
                    .map(|definition| infer_to_type(builtin_value_type(definition.ty)))
            })
    }
}

fn annotated_binary(operator: &mut BinOp, left: &Type, right: &Type) -> Type {
    match operator {
        BinOp::Add if *left == Type::Str && *right == Type::Str => {
            *operator = BinOp::StrConcat;
            Type::Str
        }
        BinOp::Mul => multiply_type(left, right),
        BinOp::Add | BinOp::Sub | BinOp::RemFloor | BinOp::RemTrunc => numeric_type(left, right),
        BinOp::DivInt if *left == Type::Float || *right == Type::Float => {
            *operator = BinOp::DivFloat;
            Type::Float
        }
        BinOp::DivInt => Type::Int,
        BinOp::DivFloat => Type::Float,
        BinOp::Eq
        | BinOp::NotEq
        | BinOp::Less
        | BinOp::LessEq
        | BinOp::Greater
        | BinOp::GreaterEq
        | BinOp::And
        | BinOp::Or => Type::Bool,
        BinOp::StrConcat => Type::Str,
    }
}

fn numeric_type(left: &Type, right: &Type) -> Type {
    if *left == Type::Float || *right == Type::Float {
        Type::Float
    } else {
        Type::Int
    }
}

fn multiply_type(left: &Type, right: &Type) -> Type {
    match (left, right) {
        (Type::Matrix(left), Type::Matrix(right)) if left == right => Type::Matrix(*left),
        (Type::Matrix(matrix), Type::Vector(vector)) if matrix == vector => Type::Vector(*vector),
        (Type::Vector(size), Type::Int | Type::Float)
        | (Type::Int | Type::Float, Type::Vector(size)) => Type::Vector(*size),
        _ => numeric_type(left, right),
    }
}

fn join_types(left: Type, right: Type) -> Type {
    match (left, right) {
        (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
        (left, right) if left == right => left,
        (Type::Option(left), Type::Option(right)) => {
            Type::Option(Box::new(join_types(*left, *right)))
        }
        (left, _) => left,
    }
}

fn infer_to_type(ty: InferType) -> Type {
    match ty {
        InferType::Unit => Type::Unit,
        InferType::Int => Type::Int,
        InferType::Float => Type::Float,
        InferType::Bool => Type::Bool,
        InferType::Str => Type::Str,
        InferType::Struct(name) => Type::Struct(name),
        InferType::Vector(size) => Type::Vector(size),
        InferType::Matrix(size) => Type::Matrix(size),
        InferType::Opaque(kind) => Type::Opaque(kind),
        InferType::Array(value) => Type::Array(Box::new(infer_to_type(*value))),
        InferType::Map(value) => Type::Map(Box::new(infer_to_type(*value))),
        InferType::Option(value) => Type::Option(Box::new(infer_to_type(*value))),
        InferType::Var(_) | InferType::ShaderValue | InferType::Error => Type::Unit,
    }
}

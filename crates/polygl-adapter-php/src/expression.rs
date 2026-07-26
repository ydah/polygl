use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, ArgumentList, Array, ArrayElement, Binary, BinaryOperator, Call,
    ClassLikeMemberSelector, Expression, Instantiation, Literal as PhpLiteral, UnaryPrefixOperator,
    Variable,
};
use polygl_adapter_api::{constructor_function_name, vector_constructor_size};
use polygl_hir::{
    BinOp, Callee, Expr, ExprKind, Literal, MapEntry, Symbol, TypeExpr, TypeKind, UnOp,
};
use polygl_span::{Diagnostic, Severity, Suggestion};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_expression(&mut self, expression: &Expression<'_>) -> Option<Expr> {
        let span = self.span(expression.span());
        let kind = match expression {
            Expression::Literal(literal) => self.lower_literal(literal)?,
            Expression::Variable(Variable::Direct(variable)) => {
                let name = self.variable_name(variable.name);
                if name == "this" {
                    if self.current_class.is_none() {
                        self.unsupported_with_code(
                            expression.span(),
                            "E0203",
                            "`$this` is only available inside a Common Core class method",
                            "use an explicit function parameter",
                        );
                        return None;
                    }
                    return Some(Expr::new(ExprKind::Var(Symbol::new("self")), span));
                }
                if !self.declared.contains(&name) {
                    self.unsupported(
                        expression.span(),
                        "this PHP local is not declared in the current Common Core block",
                        "assign the local before reading it",
                    );
                    return None;
                }
                ExprKind::Var(Symbol::new(name))
            }
            Expression::Variable(_) => {
                self.unsupported(
                    expression.span(),
                    "variable variables are outside Common Core",
                    "use a directly named local variable",
                );
                return None;
            }
            Expression::ConstantAccess(constant) => {
                if !constant.name.is_local() {
                    self.unsupported(
                        constant.span(),
                        "namespaced constants violate the single-file Common Core",
                        "use a top-level constant declared in this source file",
                    );
                    return None;
                }
                let name = self.name(constant.name.value());
                if !self.constant_names.contains(&name) {
                    self.unsupported(
                        constant.span(),
                        "this constant is not declared in the current Common Core file",
                        "declare it with a top-level `const` statement",
                    );
                    return None;
                }
                ExprKind::Var(Symbol::new(name))
            }
            Expression::Parenthesized(parenthesized) => {
                return self.lower_expression(parenthesized.expression);
            }
            Expression::Binary(binary) => return self.lower_binary(binary),
            Expression::UnaryPrefix(unary) => {
                if matches!(unary.operator, UnaryPrefixOperator::Negation(_))
                    && matches!(
                        unary.operand,
                        Expression::Literal(PhpLiteral::Integer(integer))
                            if integer.value == Some((i32::MAX as u64) + 1)
                    )
                {
                    return Some(Expr::new(ExprKind::Literal(Literal::Int(i32::MIN)), span));
                }
                let operand = self.lower_expression(unary.operand)?;
                match unary.operator {
                    UnaryPrefixOperator::Negation(_) => ExprKind::Unary {
                        op: UnOp::Neg,
                        operand: Box::new(operand),
                    },
                    UnaryPrefixOperator::Not(_) => ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(operand),
                    },
                    UnaryPrefixOperator::Plus(_) => return Some(operand),
                    _ => {
                        self.unsupported(
                            expression.span(),
                            "this PHP unary operator or cast is outside Common Core",
                            "use `-`, `!`, or an explicit Common Core conversion function",
                        );
                        return None;
                    }
                }
            }
            Expression::Array(array) => return self.lower_array(array),
            Expression::LegacyArray(_) | Expression::List(_) => {
                self.unsupported(
                    expression.span(),
                    "legacy array and list syntax are outside Common Core",
                    "use a `[...]` array or string-keyed map literal",
                );
                return None;
            }
            Expression::ArrayAccess(access) => ExprKind::Index {
                base: Box::new(self.lower_expression(access.array)?),
                index: Box::new(self.lower_expression(access.index)?),
            },
            Expression::Access(Access::Property(access)) => {
                let ClassLikeMemberSelector::Identifier(field) = &access.property else {
                    self.unsupported_with_code(
                        access.property.span(),
                        "E0203",
                        "dynamic PHP property names are outside Common Core",
                        "access a directly named field",
                    );
                    return None;
                };
                let name = self.name(field.value);
                if !self.field_names.contains(&name) && !is_builtin_event_field(&name) {
                    self.unsupported_with_code(
                        field.span(),
                        "E0203",
                        "this field is not established by a Common Core constructor",
                        "assign the field once in `__construct` before reading it",
                    );
                    return None;
                }
                ExprKind::Field {
                    base: Box::new(self.lower_expression(access.object)?),
                    field: Symbol::new(name),
                }
            }
            Expression::Access(_) => {
                self.unsupported_with_code(
                    expression.span(),
                    "E0203",
                    "null-safe, static, and class-constant access are outside Common Core",
                    "use a direct instance field after an explicit null check",
                );
                return None;
            }
            Expression::Call(call) => return self.lower_call(call),
            Expression::Instantiation(instantiation) => {
                return self.lower_instantiation(instantiation);
            }
            Expression::Closure(_) | Expression::ArrowFunction(_) => {
                self.unsupported_with_code(
                    expression.span(),
                    "E0202",
                    "PHP closures and arrow functions create values outside Common Core",
                    "replace the closure with a top-level function and call it directly",
                );
                return None;
            }
            Expression::Assignment(_) => {
                self.unsupported(
                    expression.span(),
                    "assignment expressions are only accepted as statements",
                    "move the assignment to its own statement",
                );
                return None;
            }
            _ => {
                self.unsupported(
                    expression.span(),
                    "this PHP expression is outside Common Core",
                    "rewrite it using literals, locals, operators, arrays, maps, or function calls",
                );
                return None;
            }
        };
        Some(Expr::new(kind, span))
    }

    pub(crate) fn lower_expression_with_expected(
        &mut self,
        expression: &Expression<'_>,
        expected: Option<&TypeExpr>,
    ) -> Option<Expr> {
        if matches!(expected.map(|ty| &ty.kind), Some(TypeKind::Map(_)))
            && matches!(expression, Expression::Array(array) if array.elements.is_empty())
        {
            return Some(Expr::new(
                ExprKind::Map(Vec::new()),
                self.span(expression.span()),
            ));
        }
        self.lower_expression(expression)
    }

    fn lower_literal(&mut self, literal: &PhpLiteral<'_>) -> Option<ExprKind> {
        match literal {
            PhpLiteral::Integer(integer) => {
                let Some(value) = integer.value.and_then(|value| i32::try_from(value).ok()) else {
                    let span = self.span(integer.span());
                    self.diagnostics.push(
                        Diagnostic::new(
                            Severity::Error,
                            "E0300",
                            "integer literal is outside the Common Core i32 range",
                            span,
                        )
                        .with_suggestion(Suggestion::rewrite(
                            span,
                            "use a value from -2147483648 through 2147483647",
                        )),
                    );
                    return None;
                };
                Some(ExprKind::Literal(Literal::Int(value)))
            }
            PhpLiteral::Float(float) => {
                Some(ExprKind::Literal(Literal::Float(float.value.into_inner())))
            }
            PhpLiteral::String(string) => {
                let Some(value) = string
                    .value
                    .and_then(|value| std::str::from_utf8(value).ok())
                else {
                    self.unsupported(
                        string.span(),
                        "binary or invalid PHP strings are outside Common Core",
                        "use a UTF-8 string literal without interpolation",
                    );
                    return None;
                };
                Some(ExprKind::Literal(Literal::Str(value.to_owned())))
            }
            PhpLiteral::True(_) => Some(ExprKind::Literal(Literal::Bool(true))),
            PhpLiteral::False(_) => Some(ExprKind::Literal(Literal::Bool(false))),
            PhpLiteral::Null(_) => Some(ExprKind::Literal(Literal::None)),
        }
    }

    fn lower_binary(&mut self, binary: &Binary<'_>) -> Option<Expr> {
        let span = self.span(binary.span());
        if matches!(
            binary.operator,
            BinaryOperator::Identical(_) | BinaryOperator::NotIdentical(_)
        ) && let Some(value) = null_comparison_value(binary)
        {
            let check = Expr::new(
                ExprKind::NilCheck(Box::new(self.lower_expression(value)?)),
                span,
            );
            if matches!(binary.operator, BinaryOperator::NotIdentical(_)) {
                return Some(Expr::new(
                    ExprKind::Unary {
                        op: UnOp::Not,
                        operand: Box::new(check),
                    },
                    span,
                ));
            }
            return Some(check);
        }
        let op = match binary.operator {
            BinaryOperator::Addition(_) => BinOp::Add,
            BinaryOperator::Subtraction(_) => BinOp::Sub,
            BinaryOperator::Multiplication(_) => BinOp::Mul,
            BinaryOperator::Division(_) => BinOp::DivFloat,
            BinaryOperator::Modulo(_) => BinOp::RemTrunc,
            BinaryOperator::Identical(_) => BinOp::Eq,
            BinaryOperator::NotIdentical(_) => BinOp::NotEq,
            BinaryOperator::LessThan(_) => BinOp::Less,
            BinaryOperator::LessThanOrEqual(_) => BinOp::LessEq,
            BinaryOperator::GreaterThan(_) => BinOp::Greater,
            BinaryOperator::GreaterThanOrEqual(_) => BinOp::GreaterEq,
            BinaryOperator::StringConcat(_) => BinOp::StrConcat,
            BinaryOperator::And(_) => BinOp::And,
            BinaryOperator::Or(_) => BinOp::Or,
            BinaryOperator::Equal(operator)
            | BinaryOperator::NotEqual(operator)
            | BinaryOperator::AngledNotEqual(operator) => {
                let span = self.span(operator);
                let replacement = if matches!(binary.operator, BinaryOperator::Equal(_)) {
                    "==="
                } else {
                    "!=="
                };
                self.diagnostics.push(
                    Diagnostic::new(
                        Severity::Error,
                        "E0302",
                        "PHP loose equality is outside Common Core",
                        span,
                    )
                    .with_suggestion(Suggestion::new(
                        span,
                        replacement,
                        "use strict same-type equality",
                    )),
                );
                return None;
            }
            _ => {
                self.unsupported(
                    binary.operator.span(),
                    "this PHP binary operator is outside Common Core",
                    "use arithmetic, strict comparison, boolean operators, or string concatenation",
                );
                return None;
            }
        };
        let left = self.lower_expression(binary.lhs)?;
        let right = self.lower_expression(binary.rhs)?;
        Some(Expr::new(
            ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        ))
    }

    fn lower_array(&mut self, array: &Array<'_>) -> Option<Expr> {
        let span = self.span(array.span());
        let has_keys = array
            .elements
            .iter()
            .any(|element| matches!(element, ArrayElement::KeyValue(_)));
        let has_values = array
            .elements
            .iter()
            .any(|element| matches!(element, ArrayElement::Value(_)));
        if has_keys && has_values {
            self.unsupported(
                array.span(),
                "mixed keyed and positional PHP arrays are outside Common Core",
                "use either a positional array or a string-keyed map",
            );
            return None;
        }
        if has_keys {
            let mut entries = Vec::new();
            for element in array.elements.iter() {
                let ArrayElement::KeyValue(element) = element else {
                    self.unsupported(
                        element.span(),
                        "array unpacking and missing elements are outside Common Core",
                        "list each string-keyed map entry explicitly",
                    );
                    return None;
                };
                entries.push(MapEntry {
                    key: self.lower_expression(element.key)?,
                    value: self.lower_expression(element.value)?,
                    span: self.span(element.span()),
                });
            }
            Some(Expr::new(ExprKind::Map(entries), span))
        } else {
            let mut items = Vec::new();
            for element in array.elements.iter() {
                let ArrayElement::Value(element) = element else {
                    self.unsupported(
                        element.span(),
                        "array unpacking and missing elements are outside Common Core",
                        "list each array element explicitly",
                    );
                    return None;
                };
                items.push(self.lower_expression(element.value)?);
            }
            Some(Expr::new(ExprKind::Array(items), span))
        }
    }

    fn lower_call(&mut self, call: &Call<'_>) -> Option<Expr> {
        if let Call::Method(call) = call {
            let ClassLikeMemberSelector::Identifier(method) = &call.method else {
                self.unsupported_with_code(
                    call.method.span(),
                    "E0203",
                    "dynamic PHP method names are outside Common Core",
                    "call a directly named instance method",
                );
                return None;
            };
            let name = self.name(method.value);
            if !self
                .class_methods
                .values()
                .any(|methods| methods.contains(&name))
            {
                self.unsupported_with_code(
                    method.span(),
                    "E0203",
                    "this method is not declared by a Common Core class",
                    "declare an instance method or use a plain function call",
                );
                return None;
            }
            let mut args = Vec::new();
            args.push(self.lower_expression(call.object)?);
            args.extend(self.lower_argument_list(&call.argument_list)?);
            return Some(Expr::new(
                ExprKind::Call {
                    callee: Callee::Method(Symbol::new(name)),
                    args,
                },
                self.span(call.span()),
            ));
        }
        let Call::Function(call) = call else {
            self.unsupported_with_code(
                call.span(),
                "E0203",
                "null-safe and static method calls are outside Common Core",
                "use a direct instance method after an explicit null check",
            );
            return None;
        };
        let Expression::Identifier(identifier) = call.function else {
            self.unsupported(
                call.function.span(),
                "dynamic PHP function calls are outside Common Core",
                "call a directly named function",
            );
            return None;
        };
        if !identifier.is_local() {
            self.unsupported(
                identifier.span(),
                "namespaced function calls violate the single-file Common Core",
                "call a function declared in this source file",
            );
            return None;
        }
        let name = self.name(identifier.value());
        let mut args = self.lower_argument_list(&call.argument_list)?;
        let span = self.span(call.span());
        if name == "is_null" && args.len() == 1 {
            return Some(Expr::new(
                ExprKind::NilCheck(Box::new(args.remove(0))),
                span,
            ));
        }
        if name == "count" && args.len() == 1 {
            return Some(Expr::new(
                ExprKind::ArrayLength(Box::new(args.remove(0))),
                span,
            ));
        }
        if let Some(size) = vector_constructor_size(&name) {
            return Some(Expr::new(ExprKind::Vector { size, args }, span));
        }
        let callee = self
            .context
            .resolve_builtin(&name)
            .map_or_else(|| Callee::User(Symbol::new(name)), Callee::Builtin);
        Some(Expr::new(ExprKind::Call { callee, args }, span))
    }

    fn lower_instantiation(&mut self, instantiation: &Instantiation<'_>) -> Option<Expr> {
        let Expression::Identifier(class) = instantiation.class else {
            self.unsupported_with_code(
                instantiation.class.span(),
                "E0203",
                "dynamic and anonymous class construction is outside Common Core",
                "instantiate a directly named class declared in this source file",
            );
            return None;
        };
        if !class.is_local() {
            self.unsupported_with_code(
                class.span(),
                "E0203",
                "namespaced class construction violates the single-file Common Core",
                "instantiate a top-level class declared in this source file",
            );
            return None;
        }
        let class_name = self.name(class.value());
        if !self.class_names.contains(&class_name) {
            self.unsupported_with_code(
                class.span(),
                "E0203",
                "construction is limited to classes declared in this source file",
                "declare a struct-like class before using `new`",
            );
            return None;
        }
        let args = instantiation
            .argument_list
            .as_ref()
            .map_or(Some(Vec::new()), |arguments| {
                self.lower_argument_list(arguments)
            })?;
        Some(Expr::new(
            ExprKind::Call {
                callee: Callee::User(Symbol::new(constructor_function_name(&class_name))),
                args,
            },
            self.span(instantiation.span()),
        ))
    }

    fn lower_argument_list(&mut self, arguments: &ArgumentList<'_>) -> Option<Vec<Expr>> {
        let mut result = Vec::new();
        for argument in arguments.arguments.iter() {
            let Argument::Positional(argument) = argument else {
                self.unsupported(
                    argument.span(),
                    "named arguments are outside Common Core",
                    "pass arguments positionally",
                );
                return None;
            };
            if argument.ellipsis.is_some() {
                self.unsupported(
                    argument.span(),
                    "argument unpacking is outside Common Core",
                    "pass each argument explicitly",
                );
                return None;
            }
            result.push(self.lower_expression(argument.value)?);
        }
        Some(result)
    }
}

fn is_builtin_event_field(name: &str) -> bool {
    matches!(name, "kind" | "x" | "y" | "key")
}

fn null_comparison_value<'arena>(binary: &Binary<'arena>) -> Option<&'arena Expression<'arena>> {
    let lhs = without_parentheses(binary.lhs);
    let rhs = without_parentheses(binary.rhs);
    if matches!(lhs, Expression::Literal(PhpLiteral::Null(_))) {
        return Some(rhs);
    }
    if matches!(rhs, Expression::Literal(PhpLiteral::Null(_))) {
        return Some(lhs);
    }
    None
}

fn without_parentheses<'arena>(
    mut expression: &'arena Expression<'arena>,
) -> &'arena Expression<'arena> {
    while let Expression::Parenthesized(parenthesized) = expression {
        expression = parenthesized.expression;
    }
    expression
}

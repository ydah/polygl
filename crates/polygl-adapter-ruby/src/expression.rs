use polygl_adapter_api::{constructor_function_name, vector_constructor_size};
use polygl_hir::{BinOp, Callee, Expr, ExprKind, Literal, MapEntry, Symbol};
use ruby_prism::{ArrayNode, CallNode, HashNode, Node};

use crate::lowerer::Lowerer;
use crate::operator::{binary_operator, unary_operator};

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_expression(&mut self, node: &Node<'_>) -> Option<Expr> {
        let span = self.span(node.location());
        let kind = if let Some(integer) = node.as_integer_node() {
            self.lower_integer_literal(node, &integer)?
        } else if let Some(float) = node.as_float_node() {
            ExprKind::Literal(Literal::Float(float.value()))
        } else if let Some(string) = node.as_string_node() {
            match std::str::from_utf8(string.unescaped()) {
                Ok(value) => ExprKind::Literal(Literal::Str(value.to_owned())),
                Err(_) => {
                    self.unsupported(
                        node,
                        "binary Ruby strings are outside Common Core",
                        "use a UTF-8 string literal",
                    );
                    return None;
                }
            }
        } else if let Some(symbol) = node.as_symbol_node() {
            match std::str::from_utf8(symbol.unescaped()) {
                Ok(value) => ExprKind::Literal(Literal::Str(value.to_owned())),
                Err(_) => {
                    self.unsupported(
                        node,
                        "binary Ruby symbols are outside Common Core",
                        "use a UTF-8 string literal",
                    );
                    return None;
                }
            }
        } else if node.as_true_node().is_some() {
            ExprKind::Literal(Literal::Bool(true))
        } else if node.as_false_node().is_some() {
            ExprKind::Literal(Literal::Bool(false))
        } else if node.as_nil_node().is_some() {
            ExprKind::Literal(Literal::None)
        } else if node.as_self_node().is_some() {
            if self.current_class.is_none() {
                self.unsupported(
                    node,
                    "`self` values are only available inside Common Core class methods",
                    "use an explicit function parameter",
                );
                return None;
            }
            ExprKind::Var(Symbol::new("self"))
        } else if let Some(variable) = node.as_instance_variable_read_node() {
            if self.current_class.is_none() {
                self.unsupported_with_code(
                    node,
                    "E0203",
                    "instance fields are only available inside a Common Core class",
                    "move this state into a struct-like class or a local variable",
                );
                return None;
            }
            ExprKind::Field {
                base: Box::new(Expr::new(ExprKind::Var(Symbol::new("self")), span)),
                field: Symbol::new(instance_field_name(self, variable.name().as_slice())),
            }
        } else if let Some(variable) = node.as_local_variable_read_node() {
            let name = self.name(variable.name().as_slice());
            if !self.declared.contains(&name) {
                self.unsupported(
                    node,
                    "this Ruby local is not declared in the current Common Core block",
                    "initialize the local before entering nested control flow",
                );
                return None;
            }
            ExprKind::Var(Symbol::new(name))
        } else if let Some(array) = node.as_array_node() {
            return self.lower_array(&array);
        } else if let Some(hash) = node.as_hash_node() {
            return self.lower_hash(&hash);
        } else if let Some(call) = node.as_call_node() {
            return self.lower_call(&call);
        } else if let Some(and) = node.as_and_node() {
            return self.lower_binary(BinOp::And, &and.left(), &and.right(), span);
        } else if let Some(or) = node.as_or_node() {
            return self.lower_binary(BinOp::Or, &or.left(), &or.right(), span);
        } else if let Some(parentheses) = node.as_parentheses_node() {
            return self.lower_parentheses(parentheses.body(), node);
        } else {
            self.unsupported(
                node,
                "this Ruby expression is outside Common Core",
                "rewrite it using literals, local variables, operators, or function calls",
            );
            return None;
        };
        Some(Expr::new(kind, span))
    }

    fn lower_call(&mut self, call: &CallNode<'_>) -> Option<Expr> {
        let node = call.as_node();
        let span = self.span(call.location());
        let name = self.name(call.name().as_slice());
        if call.block().is_some() {
            self.unsupported_with_code(
                &node,
                "E0202",
                "Ruby blocks are only supported as direct `times` or `each` statements",
                "rewrite this expression as a direct loop or move the block body into a plain function",
            );
            return None;
        }

        if let Some(receiver) = call.receiver() {
            let arguments = call
                .arguments()
                .map_or_else(Vec::new, |arguments| arguments.arguments().iter().collect());
            if name == "new"
                && let Some(class) = receiver.as_constant_read_node()
            {
                let class_name = self.name(class.name().as_slice());
                if self.class_names.contains(&class_name) {
                    let args = self.lower_arguments(call)?;
                    return Some(Expr::new(
                        ExprKind::Call {
                            callee: Callee::User(Symbol::new(constructor_function_name(
                                &class_name,
                            ))),
                            args,
                        },
                        span,
                    ));
                }
                self.unsupported_with_code(
                    &node,
                    "E0203",
                    "construction is limited to classes declared in this source file",
                    "declare a struct-like class before calling `.new`",
                );
                return None;
            }
            if name == "[]" && arguments.len() == 1 {
                let base = self.lower_expression(&receiver)?;
                let index = self.lower_expression(&arguments[0])?;
                return Some(Expr::new(
                    ExprKind::Index {
                        base: Box::new(base),
                        index: Box::new(index),
                    },
                    span,
                ));
            }
            if let Some(operator) = binary_operator(&name)
                && arguments.len() == 1
            {
                return self.lower_binary(operator, &receiver, &arguments[0], span);
            }
            if let Some(operator) = unary_operator(&name)
                && arguments.is_empty()
            {
                let operand = self.lower_expression(&receiver)?;
                return Some(Expr::new(
                    ExprKind::Unary {
                        op: operator,
                        operand: Box::new(operand),
                    },
                    span,
                ));
            }
            if name == "!" && arguments.is_empty() {
                let operand = self.lower_expression(&receiver)?;
                return Some(Expr::new(ExprKind::FalsyCheck(Box::new(operand)), span));
            }
            if arguments.is_empty()
                && !call_has_parentheses(call)
                && (is_builtin_event_field(&name) || self.field_names.contains(&name))
            {
                let base = self.lower_expression(&receiver)?;
                return Some(Expr::new(
                    ExprKind::Field {
                        base: Box::new(base),
                        field: Symbol::new(name),
                    },
                    span,
                ));
            }
            if call.is_safe_navigation() {
                self.unsupported_with_code(
                    &node,
                    "E0203",
                    "safe-navigation dispatch is outside the static class subset",
                    "test for `nil` explicitly before calling the method",
                );
                return None;
            }
            if receiver.as_constant_read_node().is_some() {
                self.unsupported_with_code(
                    &node,
                    "E0203",
                    "static class members are outside Common Core",
                    "replace the static member with a top-level function",
                );
                return None;
            }
            if !self
                .class_methods
                .values()
                .any(|methods| methods.contains(&name))
            {
                self.unsupported(
                    &node,
                    "this receiver method is not declared by a Common Core class",
                    "declare an instance method or use a plain function call",
                );
                return None;
            }
            let mut args = Vec::with_capacity(arguments.len() + 1);
            args.push(self.lower_expression(&receiver)?);
            for argument in arguments {
                args.push(self.lower_expression(&argument)?);
            }
            return Some(Expr::new(
                ExprKind::Call {
                    callee: Callee::Method(Symbol::new(name)),
                    args,
                },
                span,
            ));
        }

        if name == "define_method" {
            self.unsupported(
                &node,
                "`define_method` is outside Common Core",
                "use a regular `def name` declaration",
            );
            return None;
        }

        let mut args = self.lower_arguments(call)?;
        if let Some(class_name) = &self.current_class
            && self
                .class_methods
                .get(class_name)
                .is_some_and(|methods| methods.contains(&name))
        {
            args.insert(0, Expr::new(ExprKind::Var(Symbol::new("self")), span));
            return Some(Expr::new(
                ExprKind::Call {
                    callee: Callee::Method(Symbol::new(name)),
                    args,
                },
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

    fn lower_arguments(&mut self, call: &CallNode<'_>) -> Option<Vec<Expr>> {
        let mut result = Vec::new();
        if let Some(arguments) = call.arguments() {
            for argument in arguments.arguments().iter() {
                result.push(self.lower_expression(&argument)?);
            }
        }
        Some(result)
    }

    fn lower_array(&mut self, array: &ArrayNode<'_>) -> Option<Expr> {
        let node = array.as_node();
        if array.is_contains_splat() {
            self.unsupported(
                &node,
                "array splats are outside Common Core",
                "list each array element explicitly",
            );
            return None;
        }
        let mut items = Vec::new();
        for element in array.elements().iter() {
            items.push(self.lower_expression(&element)?);
        }
        Some(Expr::new(
            ExprKind::Array(items),
            self.span(array.location()),
        ))
    }

    fn lower_hash(&mut self, hash: &HashNode<'_>) -> Option<Expr> {
        let mut entries = Vec::new();
        for element in hash.elements().iter() {
            let Some(association) = element.as_assoc_node() else {
                self.unsupported(
                    &element,
                    "hash splats are outside Common Core",
                    "list each string-keyed hash entry explicitly",
                );
                return None;
            };
            let key_node = association.key();
            let key = if let Some(symbol) = key_node.as_symbol_node() {
                match std::str::from_utf8(symbol.unescaped()) {
                    Ok(value) => Expr::new(
                        ExprKind::Literal(Literal::Str(value.to_owned())),
                        self.span(symbol.location()),
                    ),
                    Err(_) => {
                        self.unsupported(
                            &key_node,
                            "binary Ruby symbols cannot be Common Core map keys",
                            "use a UTF-8 string key",
                        );
                        return None;
                    }
                }
            } else {
                self.lower_expression(&key_node)?
            };
            let value = self.lower_expression(&association.value())?;
            entries.push(MapEntry {
                key,
                value,
                span: self.span(association.location()),
            });
        }
        Some(Expr::new(
            ExprKind::Map(entries),
            self.span(hash.location()),
        ))
    }

    fn lower_parentheses(&mut self, body: Option<Node<'_>>, parent: &Node<'_>) -> Option<Expr> {
        let Some(body) = body else {
            self.unsupported(
                parent,
                "empty parentheses are outside Common Core",
                "place one expression inside the parentheses",
            );
            return None;
        };
        if let Some(statements) = body.as_statements_node() {
            let nodes = statements.body();
            if nodes.len() == 1 {
                return self.lower_expression(&nodes.first().expect("length was checked"));
            }
        }
        self.unsupported(
            parent,
            "multiple expressions in parentheses are outside Common Core",
            "keep one expression inside the parentheses",
        );
        None
    }
}

fn instance_field_name(lowerer: &Lowerer<'_, '_, '_>, name: &[u8]) -> String {
    lowerer
        .name(name)
        .strip_prefix('@')
        .unwrap_or_default()
        .to_owned()
}

fn call_has_parentheses(call: &CallNode<'_>) -> bool {
    call.opening_loc().is_some()
        || call.closing_loc().is_some()
        || call.location().as_slice().ends_with(b")")
}

fn is_builtin_event_field(name: &str) -> bool {
    matches!(name, "kind" | "x" | "y" | "key")
}

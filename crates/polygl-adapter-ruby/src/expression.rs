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
            if arguments.is_empty() && is_builtin_event_field(&name) {
                let base = self.lower_expression(&receiver)?;
                return Some(Expr::new(
                    ExprKind::Field {
                        base: Box::new(base),
                        field: Symbol::new(name),
                    },
                    span,
                ));
            }
            self.unsupported(
                &node,
                "Ruby method dispatch is outside Common Core",
                "use an operator or a plain function call without a receiver",
            );
            return None;
        }

        if name == "define_method" {
            self.unsupported(
                &node,
                "`define_method` is outside Common Core",
                "use a regular `def name` declaration",
            );
            return None;
        }

        let args = self.lower_arguments(call)?;
        if let Some(size) = vector_size(&name) {
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

fn vector_size(name: &str) -> Option<u8> {
    match name {
        "vec2" => Some(2),
        "vec3" => Some(3),
        "vec4" => Some(4),
        _ => None,
    }
}

fn is_builtin_event_field(name: &str) -> bool {
    matches!(name, "kind" | "x" | "y" | "key")
}

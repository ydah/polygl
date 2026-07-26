use polygl_hir::{BinOp, Callee, Expr, ExprKind, Literal, Symbol};
use ruby_prism::{CallNode, Node};

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
            let suggestion = if name == "times" || name == "each" {
                "rewrite this block as a `while` loop until block sugar is enabled"
            } else {
                "move the block body into a regular function"
            };
            self.unsupported(&node, "this Ruby block is outside Common Core", suggestion);
            return None;
        }

        if let Some(receiver) = call.receiver() {
            let arguments = call
                .arguments()
                .map_or_else(Vec::new, |arguments| arguments.arguments().iter().collect());
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
            if arguments.is_empty() {
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

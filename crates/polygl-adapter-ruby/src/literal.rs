use polygl_hir::{ExprKind, Literal};
use polygl_span::{Diagnostic, Severity, Suggestion};
use ruby_prism::{IntegerNode, Node};

use crate::lowerer::Lowerer;

impl Lowerer<'_, '_, '_> {
    pub(crate) fn lower_integer_literal(
        &mut self,
        node: &Node<'_>,
        integer: &IntegerNode<'_>,
    ) -> Option<ExprKind> {
        if self.is_i32_min_literal(node) {
            return Some(ExprKind::Literal(Literal::Int(i32::MIN)));
        }
        match integer.value().try_into() {
            Ok(value) => Some(ExprKind::Literal(Literal::Int(value))),
            Err(()) => {
                let span = self.span(node.location());
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
                None
            }
        }
    }

    fn is_i32_min_literal(&self, node: &Node<'_>) -> bool {
        let span = self.span(node.location());
        let Some(text) = self.source.text().get(span.range()) else {
            return false;
        };
        let Some(magnitude) = text.strip_prefix('-') else {
            return false;
        };
        magnitude.replace('_', "") == "2147483648"
    }
}

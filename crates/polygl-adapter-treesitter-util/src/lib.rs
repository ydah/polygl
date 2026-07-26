//! Shared source, traversal, and recovery helpers for Tree-sitter adapters.

use polygl_span::{Diagnostic, Diagnostics, Severity, SourceFile, Span};
use tree_sitter::Node;

/// Converts a Tree-sitter byte range to a validated PolyGL span.
///
/// Tree-sitter grammars operate on the same UTF-8 bytes held by `SourceFile`,
/// so a successful parse must only expose valid source boundaries.
#[must_use]
pub fn node_span(source: &SourceFile, node: Node<'_>) -> Span {
    source
        .span(node.start_byte(), node.end_byte())
        .expect("Tree-sitter nodes must use source UTF-8 byte boundaries")
}

/// Returns the exact UTF-8 source slice covered by a node.
#[must_use]
pub fn node_text<'source>(source: &'source SourceFile, node: Node<'_>) -> &'source str {
    &source.text()[node.start_byte()..node.end_byte()]
}

/// Collects named structural children and drops punctuation.
#[must_use]
pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

/// Collects named children for a field while filtering grammar punctuation.
///
/// This defensive filter is required by `ts-parser-perl` 1.2.1, whose
/// parenthesized field splats may report commas and parentheses as field
/// children.
#[must_use]
pub fn named_field_children<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .filter(Node::is_named)
        .collect()
}

/// Returns the first named field child, including a structural fallback for
/// grammars affected by field-splat bugs.
#[must_use]
pub fn first_named_field<'tree>(node: Node<'tree>, field: &str) -> Option<Node<'tree>> {
    node.child_by_field_name(field)
        .filter(Node::is_named)
        .or_else(|| named_field_children(node, field).into_iter().next())
}

/// Converts every Tree-sitter recovery node into a source-spanned E0100.
///
/// Adapters must reject recovered trees rather than lowering a plausible but
/// incomplete interpretation of invalid source.
#[must_use]
pub fn recovery_diagnostics(source: &SourceFile, root: Node<'_>) -> Diagnostics {
    let mut diagnostics = Diagnostics::new();
    if !root.has_error() {
        return diagnostics;
    }

    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            let message = if node.is_missing() {
                format!("parser expected `{}` here", node.kind())
            } else {
                "parser could not recognize this syntax".to_owned()
            };
            diagnostics.push(Diagnostic::new(
                Severity::Error,
                "E0100",
                message,
                node_span(source, node),
            ));
        }
        stack.extend(named_children(node));
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use polygl_span::{SourceFile, SourceId};
    use tree_sitter::Parser;

    use super::{
        first_named_field, named_field_children, node_span, node_text, recovery_diagnostics,
    };

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = Parser::new();
        parser
            .set_language(&ts_parser_perl::LANGUAGE.into())
            .expect("load Perl grammar");
        parser.parse(source, None).expect("parse source")
    }

    #[test]
    fn converts_utf8_ranges_and_rejects_recovery() {
        let source = SourceFile::new(
            SourceId::new(3),
            "unicode.pl",
            "sub setup { my $label = \"雪\"; }",
        );
        let tree = parse(source.text());
        let root = tree.root_node();
        assert!(!root.has_error());
        assert_eq!(node_span(&source, root).end(), source.len());
        assert_eq!(node_text(&source, root), source.text());
        assert!(recovery_diagnostics(&source, root).is_empty());

        let invalid = SourceFile::new(SourceId::new(4), "broken.pl", "sub setup { my $x = ;");
        let tree = parse(invalid.text());
        let diagnostics = recovery_diagnostics(&invalid, tree.root_node());
        assert!(diagnostics.has_errors());
        assert!(diagnostics.iter().all(|item| item.code == "E0100"));
    }

    #[test]
    fn filters_parenthesized_perl_field_splat_punctuation() {
        let source = SourceFile::new(
            SourceId::new(5),
            "parameters.pl",
            "sub move { my ($self, $dx, $dy) = @_; }",
        );
        let tree = parse(source.text());
        let root = tree.root_node();
        let subroutine = root.named_child(0).expect("subroutine");
        let body = first_named_field(subroutine, "body").expect("body");
        let assignment = body
            .named_child(0)
            .and_then(|statement| statement.named_child(0))
            .expect("parameter assignment");
        let declaration = first_named_field(assignment, "left").expect("declaration");

        let variables = named_field_children(declaration, "variables");
        assert_eq!(variables.len(), 3);
        assert!(variables.iter().all(tree_sitter::Node::is_named));
    }
}

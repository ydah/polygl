use tree_sitter::{Node, Parser, Tree};

pub const COMMON_CORE_SOURCE: &str = include_str!("../common-core.pl");

pub fn parse(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&ts_parser_perl::LANGUAGE.into())
        .expect("load Perl grammar");
    parser.parse(source, None).expect("parse returned a tree")
}

pub fn named_field_children<'tree>(node: Node<'tree>, field: &str) -> Vec<Node<'tree>> {
    let mut cursor = node.walk();
    node.children_by_field_name(field, &mut cursor)
        .filter(Node::is_named)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_node<'tree>(
        node: Node<'tree>,
        source: &[u8],
        kind: &str,
        text_fragment: &str,
    ) -> Option<Node<'tree>> {
        if node.kind() == kind
            && node
                .utf8_text(source)
                .is_ok_and(|text| text.contains(text_fragment))
        {
            return Some(node);
        }
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find_map(|child| find_node(child, source, kind, text_fragment))
    }

    #[test]
    fn parses_the_common_core_without_recovery() {
        let tree = parse(COMMON_CORE_SOURCE);
        let root = tree.root_node();
        assert_eq!(root.kind(), "source_file");
        assert!(!root.has_error(), "{}", root.to_sexp());
        assert_eq!(root.start_byte(), 0);
        assert_eq!(root.end_byte(), COMMON_CORE_SOURCE.len());
    }

    #[test]
    fn reports_invalid_input_as_a_recovered_tree() {
        let tree = parse("sub broken { my $value = ;");
        assert!(tree.root_node().has_error());
    }

    #[test]
    fn filters_the_v1_2_1_field_splat_punctuation() {
        let tree = parse(COMMON_CORE_SOURCE);
        let source = COMMON_CORE_SOURCE.as_bytes();
        let declaration = find_node(tree.root_node(), source, "variable_declaration", "$class")
            .expect("constructor variable declaration");

        let raw = declaration
            .children_by_field_name("variables", &mut declaration.walk())
            .collect::<Vec<_>>();
        assert!(
            raw.iter().any(|child| !child.is_named()),
            "fixture must characterize the 1.2.1 field-splat bug"
        );

        let named = named_field_children(declaration, "variables");
        assert_eq!(named.len(), 3);
        assert!(named.iter().all(Node::is_named));
    }
}

//! PHP language adapter.

#[cfg(test)]
mod tests {
    use mago_allocator::LocalArena;
    use mago_database::file::FileId;
    use mago_span::HasSpan;
    use mago_syntax::comments::docblock::get_docblock_for_node;
    use mago_syntax::parser::parse_file_content;

    #[test]
    fn parser_preserves_spans_trivia_and_annotation_docblocks() {
        let source = br#"<?php
/** @pgl $x: float */
function setup() {
    $x = 1.0;
}
"#;
        let arena = LocalArena::new();
        let program = parse_file_content(&arena, FileId::zero(), source);

        assert!(program.errors.is_empty(), "{:?}", program.errors);
        let function = program
            .statements
            .iter()
            .nth(1)
            .expect("opening tag followed by function");
        let function_span = function.span();
        assert_eq!(function_span.start_offset(), 28);
        assert_eq!(function_span.end_offset(), 62);

        let docblock = get_docblock_for_node(program, function).expect("attached DocBlock");
        assert_eq!(docblock.value, b"/** @pgl $x: float */");
        assert_eq!(docblock.span().start_offset(), 6);
        assert_eq!(docblock.span().end_offset(), 27);
        assert!(!program.trivia.is_empty());
    }

    #[test]
    fn parser_reports_a_source_spanned_error() {
        let source = b"<?php function setup( {}";
        let arena = LocalArena::new();
        let program = parse_file_content(&arena, FileId::zero(), source);

        let error = program.errors.first().expect("malformed PHP must fail");
        assert!(matches!(
            error,
            mago_syntax::error::ParseError::UnexpectedToken(..)
        ));
        assert_eq!(error.span().start_offset(), 22);
        assert_eq!(error.span().end_offset(), 23);
    }
}

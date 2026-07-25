#[cfg(test)]
mod tests {
    use mago_allocator::LocalArena;
    use mago_database::file::FileId;
    use mago_span::HasSpan;
    use mago_syntax::comments::docblock::get_docblock_for_node;
    use mago_syntax::parser::parse_file_content;

    #[test]
    fn preserves_spans_trivia_and_annotation_docblocks() {
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
        assert!(function_span.start_offset() < function_span.end_offset());

        let docblock = get_docblock_for_node(program, function).expect("attached DocBlock");
        assert_eq!(docblock.value, b"/** @pgl $x: float */");
        assert!(docblock.span().start_offset() < docblock.span().end_offset());
    }
}

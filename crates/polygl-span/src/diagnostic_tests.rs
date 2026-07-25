use crate::{Diagnostic, Diagnostics, Label, Severity, SourceFile, SourceId, Suggestion};

#[test]
fn renders_codes_labels_notes_and_suggestions_without_color() {
    let source = SourceFile::new(SourceId::new(1), "main.php", "$x == 1");
    let diagnostic = Diagnostic::new(
        Severity::Error,
        "E0302",
        "loose equality is unsupported",
        source.span(3, 5).unwrap(),
    )
    .with_label(Label::new(source.span(0, 2).unwrap(), "left operand"))
    .with_note("Common Core comparisons require matching types")
    .with_suggestion(Suggestion::new(
        source.span(3, 5).unwrap(),
        "===",
        "use strict equality",
    ));

    let rendered = diagnostic.render(&source).unwrap();
    assert!(rendered.contains("E0302"));
    assert!(rendered.contains("loose equality is unsupported"));
    assert!(rendered.contains("main.php"));
    assert!(rendered.contains("left operand"));
    assert!(rendered.contains("replace with `===`"));
    assert!(!rendered.contains("\u{1b}["));
}

#[test]
fn renders_non_machine_applicable_rewrites_without_an_empty_replacement() {
    let source = SourceFile::new(SourceId::new(1), "main.rb", "define_method(:setup) {}");
    let diagnostic = Diagnostic::new(
        Severity::Error,
        "E0200",
        "dynamic methods are unsupported",
        source.span(0, source.len()).unwrap(),
    )
    .with_suggestion(Suggestion::rewrite(
        source.span(0, source.len()).unwrap(),
        "use a regular `def setup` declaration",
    ));

    let rendered = diagnostic.render(&source).unwrap();
    assert!(rendered.contains("use a regular `def setup` declaration"));
    assert!(!rendered.contains("replace with ``"));
}

#[test]
fn renders_an_empty_machine_replacement_as_a_deletion() {
    let source = SourceFile::new(SourceId::new(1), "main.rb", "unsupported");
    let diagnostic = Diagnostic::new(
        Severity::Error,
        "E0200",
        "unsupported syntax",
        source.span(0, source.len()).unwrap(),
    )
    .with_suggestion(Suggestion::new(
        source.span(0, source.len()).unwrap(),
        "",
        "remove the unsupported syntax",
    ));

    let rendered = diagnostic.render(&source).unwrap();
    assert!(rendered.contains("remove selected text"));
}

#[test]
fn rejects_labels_from_another_source_and_tracks_error_severity() {
    let source = SourceFile::new(SourceId::new(1), "main.rb", "value");
    let foreign_source = SourceFile::new(SourceId::new(2), "other.rb", "value");
    let foreign = foreign_source.span(0, 1).unwrap();
    let diagnostic = Diagnostic::new(
        Severity::Error,
        "E0100",
        "bad source",
        source.span(0, 1).unwrap(),
    )
    .with_label(Label::new(foreign, "foreign"));
    assert!(diagnostic.render(&source).is_err());

    let mut diagnostics = Diagnostics::new();
    diagnostics.push(Diagnostic::new(
        Severity::Warning,
        "W0300",
        "warning",
        source.span(0, 0).unwrap(),
    ));
    assert!(!diagnostics.has_errors());
    diagnostics.push(diagnostic);
    assert!(diagnostics.has_errors());
    assert_eq!(diagnostics.iter().len(), 2);
    let duplicate = diagnostics.iter().last().unwrap().clone();
    diagnostics.push(duplicate);
    assert_eq!(diagnostics.iter().len(), 2);
}

#[test]
fn renders_empty_eof_span_after_crlf_on_the_trailing_line() {
    let source = SourceFile::new(SourceId::new(1), "main.rb", "α\r\n");
    let eof = source.span(source.len(), source.len()).unwrap();
    let rendered = Diagnostic::new(Severity::Error, "E0100", "expected expression", eof)
        .render(&source)
        .unwrap();

    assert!(rendered.contains("main.rb:2:1"), "{rendered}");
}

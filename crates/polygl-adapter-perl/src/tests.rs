use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_core::BuiltinTable;
use polygl_hir::{Module, dump};
use polygl_span::{Diagnostics, SourceFile, SourceId};

use crate::PerlAdapter;

fn lower(source: &str) -> Result<Module, Diagnostics> {
    let source = SourceFile::new(SourceId::new(0), "main.pl", source);
    PerlAdapter.lower(&source, &mut LowerCtx::new(&BuiltinTable))
}

#[test]
fn advertises_the_static_perl_capabilities() {
    assert_eq!(PerlAdapter.id(), "perl");
    assert_eq!(PerlAdapter.file_extensions(), &["pl"]);
    for capability in [
        FeatureTag::Core,
        FeatureTag::Tier1,
        FeatureTag::Tier2,
        FeatureTag::Arrays,
        FeatureTag::Maps,
        FeatureTag::Classes,
        FeatureTag::Shaders,
    ] {
        assert!(PerlAdapter.capabilities().contains(&capability));
    }
}

#[test]
fn lowers_entries_calls_constants_and_perl_arithmetic() {
    let module = lower(
        r#"
use strict;
use warnings;
my $SCALE = 2.0;

sub helper {
    my ($value) = @_;
    return ($value / 2) % 3;
}

sub setup {
    size(8, 6);
    background(helper($SCALE), 0.25, 0.5);
}
"#,
    )
    .expect("portable Perl should lower");
    let text = dump(&module);
    assert!(text.contains("const SCALE = 2.0;"));
    assert!(text.contains("fn helper(value)"));
    assert!(text.contains("/float"));
    assert!(text.contains("%trunc"));
    assert!(text.contains("entry setup()"));
}

#[test]
fn lowers_arrays_maps_indexing_and_structured_control_flow() {
    let module = lower(
        r#"
sub values {
    my @items = (1, 2, 3);
    my %colors = ("red" => 4, blue => 5);
    my $index = 0;
    while ($index < 2) {
        $items[$index] = $items[$index] + 1;
        $index++;
    }
    for my $step (0 .. 2) {
        if ($step == 1) {
            next;
        }
        $colors{"red"} = $colors{"red"} + $step;
    }
    return $items[0] + $colors{"red"};
}
"#,
    )
    .expect("collections and loops should lower");
    let text = dump(&module);
    assert!(text.contains("let items = [1, 2, 3];"));
    assert!(text.contains("\"red\": 4"));
    assert!(text.contains("while (index < 2)"));
    assert!(text.contains("for step in 0..=2"));
    assert!(text.contains("continue;"));
}

#[test]
fn lowers_fixed_package_classes_and_static_method_dispatch() {
    let module = lower(
        r#"
package Point;

sub new {
    my ($class, $x, $y) = @_;
    my $self = { x => $x, y => $y };
    return bless $self, $class;
}

sub move {
    my ($self, $dx, $dy) = @_;
    $self->{x} = $self->{x} + $dx;
    $self->{y} = $self->{y} + $dy;
}

package main;

sub setup {
    my $point = Point->new(1.0, 2.0);
    $point->move(3.0, 4.0);
}
"#,
    )
    .expect("fixed package classes should lower");
    let text = dump(&module);
    assert!(text.contains("struct Point"));
    assert!(text.contains("fn Point::new(x, y) -> Point"));
    assert!(text.contains("fn move(self: Point, dx, dy)"));
    assert!(text.contains("Point::new"));
    assert!(text.contains(".x"));
}

#[test]
fn lowers_shader_uniform_annotations_and_vector_constructors() {
    let module = lower(
        r#"
# @pgl $tint: vec3
sub fragment_color {
    return vec4($tint, 1.0);
}
"#,
    )
    .expect("annotated shader uniform should lower");
    let text = dump(&module);
    assert!(text.contains("entry fragment_color()"));
    assert!(text.contains("uniform<tint: vec3>"), "{text}");
    assert!(text.contains("vec4(uniform<tint: vec3>, 1.0)"));
}

#[test]
fn rejects_recovered_parse_trees_with_positioned_syntax_diagnostics() {
    let diagnostics = lower("sub setup { my $value = ;").expect_err("invalid syntax must fail");
    assert!(diagnostics.has_errors());
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E0100")
    );
}

#[test]
fn requires_boolean_conditions_instead_of_perl_truthiness() {
    let source = SourceFile::new(
        SourceId::new(1),
        "truthiness.pl",
        "\n# @pgl $value: int\nsub select_value { my ($value) = @_; if ($value) { return 1; } return 0; }\nsub setup { select_value(1); }\n",
    );
    let hir = PerlAdapter
        .lower(&source, &mut LowerCtx::new(&BuiltinTable))
        .expect("adapter lowering should preserve the direct condition");
    assert!(!hir.items.is_empty(), "{}", dump(&hir));
    let diagnostics =
        polygl_types::analyze(&hir).expect_err("integer Perl truthiness must not be reproduced");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0301"
            && diagnostic.suggestion.is_some()
            && diagnostic.primary_span.start() < diagnostic.primary_span.end()
    }));
}

#[test]
fn common_core_rejections_cover_ten_perl_specific_forms_with_suggestions() {
    let cases = [
        "use Foo; sub setup { return; }",
        "require Foo; sub setup { return; }",
        "sub setup { eval { 1; }; }",
        "sub setup { goto LABEL; }",
        "sub setup { my $value = sub { return 1; }; }",
        "sub setup { my $value = qr/x/; }",
        "sub setup { my $value = do { 1; }; }",
        "sub setup { local $value = 1; }",
        "sub setup { my $value = map { $_ } (1, 2); }",
        "sub setup { until (true) { last; } }",
    ];
    assert!(cases.len() >= 10);
    for source in cases {
        let diagnostics = lower(source).expect_err(source);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code.starts_with("E02") && diagnostic.suggestion.is_some()
            }),
            "{source}: {diagnostics:?}"
        );
    }
}

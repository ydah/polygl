use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::comments::docblock::get_docblock_for_node;
use mago_syntax::parser::parse_file_content;
use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_core::BuiltinTable;
use polygl_hir::dump;
use polygl_span::{SourceFile, SourceId};

use crate::PhpAdapter;

fn lower(source: &str) -> Result<polygl_hir::Module, polygl_span::Diagnostics> {
    let source = SourceFile::new(SourceId::new(1), "main.php", source);
    PhpAdapter.lower(&source, &mut LowerCtx::new(&BuiltinTable))
}

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

#[test]
fn advertises_php_capabilities() {
    assert_eq!(PhpAdapter.id(), "php");
    assert_eq!(PhpAdapter.file_extensions(), &["php"]);
    assert_eq!(
        PhpAdapter.capabilities(),
        &[
            FeatureTag::Core,
            FeatureTag::Tier1,
            FeatureTag::Tier2,
            FeatureTag::Arrays,
            FeatureTag::Maps,
            FeatureTag::Classes,
            FeatureTag::Shaders,
        ]
    );
}

#[test]
fn lowers_functions_calls_and_php_operators() {
    let module = lower(
        r#"<?php
function helper($value) {
    return $value / 2;
}

function setup() {
    $label = "x=" . "value";
    $values = [1, 2, 3];
    $lookup = ["left" => 4, "right" => 5];
    line(helper($values[0]), $lookup["left"], 3.0, 4.0);
}
"#,
    )
    .expect("PHP Common Core should lower");
    let text = dump(&module);
    assert!(text.contains("fn helper(value)"));
    assert!(text.contains("(value /float 2)"), "{text}");
    assert!(text.contains(r#"("x=" str++ "value")"#), "{text}");
    assert!(text.contains("let values = [1, 2, 3];"), "{text}");
    assert!(text.contains(r#""left": 4"#), "{text}");
}

#[test]
fn lowers_annotated_user_and_automatic_shader_uniforms() {
    let module = lower(
        r#"<?php
/** @pgl $position: vec3 */
function vertex_textured($position) {
    return $u_proj * $u_view * $u_model * vec4($position, 1.0);
}

/** @pgl $texture_map: Texture */
function fragment_textured() {
    return sample($texture_map, vec2(0.5, 0.5));
}
"#,
    )
    .expect("PHP shader uniforms should lower");
    let text = dump(&module);
    assert!(text.contains("uniform<u_proj: mat4>"), "{text}");
    assert!(text.contains("uniform<u_model: mat4>"), "{text}");
    assert!(text.contains("uniform<texture_map: Texture>"), "{text}");
    polygl_types::analyze(&module).expect("shader uniforms should type-check");
}

#[test]
fn rejects_php_truthiness_and_loose_equality() {
    let truthiness =
        lower("<?php function setup() { $value = 1; if ($value) { line(0, 0, 1, 1); } }")
            .expect("truthiness is rejected by shared type analysis, not lowering");
    let diagnostics =
        polygl_types::analyze(&truthiness).expect_err("PHP conditions must already be bool");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == "E0301" && diagnostic.suggestion.is_some() })
    );

    let equality = lower("<?php function setup() { if (1 == 1) {} }")
        .expect_err("loose equality must be rejected");
    assert!(equality.iter().any(|diagnostic| {
        diagnostic.code == "E0302"
            && diagnostic
                .suggestion
                .as_ref()
                .and_then(|suggestion| suggestion.replacement.as_deref())
                == Some("===")
    }));
}

#[test]
fn lowers_annotations_and_struct_like_classes() {
    let module = lower(
        r#"<?php
class Point {
    /** @pgl $x: float */
    /** @pgl $label: str */
    function __construct($x, float $y, $label) {
        $this->x = $x;
        $this->y = $y;
        $this->label = $label;
    }

    function move(float $delta): float {
        $this->x = $this->x + $delta;
        return $this->x;
    }
}

function setup() {
    $point = new Point(1.0, 2.0, "origin");
    $point->move(3.0);
}
"#,
    )
    .expect("the PHP struct-like class subset should lower");
    let text = dump(&module);
    assert!(text.contains("struct Point"), "{text}");
    assert!(text.contains("field x: float;"), "{text}");
    assert!(text.contains("field y: float;"), "{text}");
    assert!(text.contains("field label: str;"), "{text}");
    assert!(
        text.contains("fn move(self: Point, delta: float) -> float"),
        "{text}"
    );
    assert!(
        text.contains("fn Point::new(x: float, y: float, label: str) -> Point"),
        "{text}"
    );
    assert!(text.contains(r#"Point::new(1.0, 2.0, "origin")"#), "{text}");
    assert!(text.contains("point.move(3.0)"), "{text}");
    polygl_types::analyze(&module).expect("class HIR should type-check");
}

#[test]
fn reports_malformed_and_unmatched_annotations() {
    for source in [
        "<?php /** @pgl $x float */ function setup() {}",
        "<?php /** @pgl $missing: float */ function setup() {}",
        "<?php /** @pgl $x: mystery */ function setup() { $x = 1; }",
    ] {
        let diagnostics = lower(source).expect_err("invalid annotations must fail");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0314" && diagnostic.suggestion.is_some()
            })
        );
    }
}

#[test]
fn lowers_constants_range_loops_foreach_and_elseif() {
    let module = lower(
        r#"<?php
const LIMIT = 3;

function setup() {
    for ($index = 0; $index < LIMIT; $index++) {
        line($index, 0, $index, 1);
    }

    $values = [1, 2, 3];
    foreach ($values as $value) {
        if ($value === 1) {
            line(0, 0, 1, 1);
        } elseif ($value === 2) {
            line(1, 1, 2, 2);
        } else {
            line(2, 2, 3, 3);
        }
    }
}
"#,
    )
    .expect("PHP control-flow forms should lower");
    let text = dump(&module);
    assert!(text.contains("const LIMIT = 3;"), "{text}");
    assert!(text.contains("for index in 0..LIMIT"), "{text}");
    assert!(text.contains("for __pgl_index_"), "{text}");
    assert!(text.matches("if (value == ").count() >= 2, "{text}");
    polygl_types::analyze(&module).expect("control-flow HIR should type-check");
}

#[test]
fn php_specific_diagnostics_cover_ten_or_more_cases() {
    for source in [
        "<?php function setup() { if (1) {} }",
        "<?php function setup() { while (\"0\") {} }",
    ] {
        let module = lower(source).expect("truthiness is a type-level error");
        let diagnostics =
            polygl_types::analyze(&module).expect_err("non-bool PHP conditions must fail");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0301"
                && !diagnostic.primary_span.is_empty()
                && diagnostic.suggestion.is_some()
        }));
    }

    for source in [
        "<?php function setup() { if (1 == 1) {} }",
        "<?php function setup() { if (1 != 1) {} }",
        "<?php function setup() { if (1 <> 1) {} }",
        "<?php function setup() { $items = [1, \"key\" => 2]; }",
        "<?php function setup() { $items = array(1, 2); }",
        "<?php function setup() { line(...[0, 0, 1, 1]); }",
        "<?php function setup() { line(x: 0, y: 0); }",
        "<?php function setup() { $name = \"x\"; ${$name} = 1; }",
        "<?php class ParentType {} class ChildType extends ParentType {}",
        "<?php class Point { public function move() {} }",
        "<?php function setup() { $value = match (1) { 1 => 2 }; }",
        "<?php function setup() { $callback = function () {}; }",
    ] {
        let diagnostics = lower(source).expect_err("unsupported PHP syntax must fail");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.suggestion.is_some()),
            "{source}"
        );
    }
}

#[test]
fn lowers_null_tests_empty_maps_void_returns_and_i32_min() {
    let module = lower(
        r#"<?php
/** @pgl $value: Option<int> */
function absent($value): bool {
    return ($value) === (null);
}

/** @pgl $value: Option<int> */
function present($value): bool {
    return $value !== null;
}

/** @pgl $value: Option<int> */
function absent_call($value): bool {
    return is_null($value);
}

function setup(): void {
    /** @pgl $lookup: Map<str, int> */
    $lookup = [];
    $minimum = -2147483648;
    for ($index = -2; $index < -1; $index++) {}
}
"#,
    )
    .expect("explicit PHP portability forms should lower");
    let text = dump(&module);
    assert!(text.matches("nil?(value)").count() >= 3, "{text}");
    assert!(
        text.contains("fn absent(value: Option<int>) -> bool"),
        "{text}"
    );
    assert!(text.contains("entry setup() -> void [host]"), "{text}");
    assert!(text.contains("let lookup: Map<str, int> = {};"), "{text}");
    assert!(text.contains("let minimum = -2147483648;"), "{text}");
    polygl_types::analyze(&module).expect("portable PHP forms should type-check");
}

#[test]
fn generated_foreach_names_are_hygienic_against_later_source_locals() {
    let module = lower(
        r#"<?php
function setup() {
    $values = [1, 2];
    foreach ($values as $value) {
        line($value, 0, $value, 1);
    }
    $__pgl_each_0 = 7;
    line($__pgl_each_0, 0, 0, 1);
}
"#,
    )
    .expect("generated names must not capture source locals");
    let text = dump(&module);
    assert!(text.contains("let __pgl_each_1 = values;"), "{text}");
    assert!(text.contains("let __pgl_each_0 = 7;"), "{text}");
    polygl_types::analyze(&module).expect("hygienic foreach HIR should type-check");
}

#[test]
fn rejects_unstable_for_bounds_and_constructor_self_reads() {
    for (source, code) in [
        (
            "<?php function setup() { $values = [1]; for ($i = 0; $i < count($values); $i++) {} }",
            "E0200",
        ),
        (
            "<?php class Pair { function __construct($a) { $this->a = $a; $this->b = $this->a; } }",
            "E0203",
        ),
    ] {
        let diagnostics = lower(source).expect_err("non-portable semantics must be rejected");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == code
                && !diagnostic.primary_span.is_empty()
                && diagnostic.suggestion.is_some()
        }));
    }
}

#[test]
fn native_hint_annotations_are_consumed_and_conflicts_are_precise() {
    lower("<?php /** @pgl $x: float */ function scale(float $x): float { return $x; }")
        .expect("a matching redundant annotation is consumed");

    let module =
        lower("<?php function move(Node $node): void { node_set_pos($node, 1.0, 2.0, 3.0); }")
            .expect("opaque handle hints should lower");
    let text = dump(&module);
    assert!(text.contains("fn move(node: Node) -> void"), "{text}");

    let diagnostics =
        lower("<?php /** @pgl $x: int */ function scale(float $x): float { return $x; }")
            .expect_err("conflicting native and portable types must fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0303"
            && !diagnostic.primary_span.is_empty()
            && diagnostic
                .suggestion
                .as_ref()
                .is_some_and(|suggestion| suggestion.replacement.as_deref() == Some(""))
    }));
}

#[test]
fn closure_and_class_feature_diagnostics_use_specific_codes() {
    for source in [
        "<?php function setup() { $callback = function () {}; }",
        "<?php function setup() { $callback = fn($x) => $x; }",
    ] {
        let diagnostics = lower(source).expect_err("closures must fail");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0202"
                && !diagnostic.primary_span.is_empty()
                && diagnostic.suggestion.is_some()
        }));
    }
    for source in [
        "<?php interface Drawable {}",
        "<?php trait Paintable {}",
        "<?php enum Color {}",
    ] {
        let diagnostics = lower(source).expect_err("class-like features must fail");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0203"
                && !diagnostic.primary_span.is_empty()
                && diagnostic.suggestion.is_some()
        }));
    }
}

#[test]
fn loose_inequality_suggestions_are_machine_applicable() {
    for (operator, replacement) in [("!=", "!=="), ("<>", "!=="), ("==", "===")] {
        let source = format!("<?php function setup() {{ if (1 {operator} 1) {{}} }}");
        let diagnostics = lower(&source).expect_err("loose equality must fail");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0302"
                && !diagnostic.primary_span.is_empty()
                && diagnostic
                    .suggestion
                    .as_ref()
                    .and_then(|suggestion| suggestion.replacement.as_deref())
                    == Some(replacement)
        }));
    }
}

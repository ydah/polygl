use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_builtins::BuiltinTable;
use polygl_hir::{EntryPointKind, Item, StmtKind, dump};
use polygl_span::{SourceFile, SourceId};

use crate::RubyAdapter;

fn lower(source: &str) -> Result<polygl_hir::Module, polygl_span::Diagnostics> {
    let source = SourceFile::new(SourceId::new(1), "main.rb", source);
    RubyAdapter.lower(&source, &mut LowerCtx::new(&BuiltinTable))
}

#[test]
fn advertises_ruby_core_capabilities() {
    let adapter = RubyAdapter;
    assert_eq!(adapter.id(), "ruby");
    assert_eq!(adapter.file_extensions(), &["rb"]);
    assert_eq!(
        adapter.capabilities(),
        &[
            FeatureTag::Core,
            FeatureTag::Tier1,
            FeatureTag::Tier2,
            FeatureTag::Arrays,
            FeatureTag::Maps,
            FeatureTag::Classes,
            FeatureTag::TimesBlockSugar,
            FeatureTag::EachBlockSugar,
            FeatureTag::TruthinessSugar,
            FeatureTag::Shaders,
        ]
    );
}

#[test]
fn lowers_struct_like_classes_constructors_fields_and_methods() {
    let module = lower(
        r#"
class Dot
  def initialize(x, y)
    @x = x
    @y = y
  end

  def move(dx)
    @x = @x + dx
    self
  end

  def paint
    circle(@x, @y, 2)
  end

  def x
    99
  end
end

def setup
  dot = Dot.new(10, 20)
  dot.move(3)
  dot.x = 15
  circle(dot.x(), dot.y, 4)
end
"#,
    )
    .expect("the struct-like class subset should lower");
    let text = dump(&module);
    assert!(text.contains("struct Dot"));
    assert!(text.contains("field x;"));
    assert!(text.contains("field y;"));
    assert!(text.contains("fn move(self: Dot, dx)"));
    assert!(text.contains("self.x = (self.x + dx);"));
    assert!(text.contains("fn Dot::new(x, y)"));
    assert!(text.contains("return Dot { x: x, y: y };"));
    assert!(text.contains("let dot = Dot::new(10, 20);"));
    assert!(text.contains("dot.move(3);"));
    assert!(text.contains("dot.x = 15;"));
    assert!(text.contains("builtin#9(dot.x(), dot.y, 4);"), "{text}");
}

#[test]
fn rejects_dynamic_class_features_with_e0203() {
    for source in [
        "class Child < Parent\nend\n",
        "class Counter\n  def self.zero\n    0\n  end\nend\n",
        "class Hidden\n  private\n  def value\n    1\n  end\nend\n",
    ] {
        let diagnostics = lower(source).expect_err("dynamic class features must be rejected");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0203" && diagnostic.suggestion.is_some()
            })
        );
    }
}

#[test]
fn rejects_instance_methods_that_conflict_with_direct_syntax_lowering() {
    for method in [
        "def ==(other)\n    false\n  end",
        "def [](index)\n    index\n  end",
        "def value=(value)\n    @value = value\n  end",
    ] {
        let source = format!("class Conflicting\n  {method}\nend\n");
        let diagnostics = lower(&source).expect_err("syntax-overloading methods must be rejected");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0203" && diagnostic.suggestion.is_some()
            }),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn every_common_core_rejection_in_the_ruby_corpus_has_a_suggestion() {
    for source in [
        "value = 1\n",
        "def helper(value = 1)\n  value\nend\n",
        "def setup\n  [1].map { |value| value }\nend\n",
        "class Child < Parent\nend\n",
        "class Counter\n  def ==(other)\n    false\n  end\nend\n",
    ] {
        let diagnostics = lower(source).expect_err("the source must be outside Common Core");
        let rejections = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.starts_with("E02"))
            .collect::<Vec<_>>();
        assert!(!rejections.is_empty(), "{diagnostics:?}");
        assert!(
            rejections
                .iter()
                .all(|diagnostic| diagnostic.suggestion.is_some()),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn lowers_arrays_maps_and_index_places() {
    let module = lower(
        r#"
def setup
  values = [1, 2, 3]
  values[0] = 4
  labels = {"left" => 5, right: 6}
  line(values[0], labels["left"], values[1], labels[:right])
end
"#,
    )
    .expect("collection literals and indexing should lower");
    let text = dump(&module);
    assert!(text.contains("let values = [1, 2, 3];"));
    assert!(text.contains("values[0] = 4;"));
    assert!(text.contains(r#"let labels = {"left": 5, "right": 6};"#));
    assert!(text.contains(r#"builtin#10(values[0], labels["left"], values[1], labels["right"]);"#));
}

#[test]
fn expands_whitelisted_blocks_to_structured_loops() {
    let module = lower(
        r#"
def setup
  total = 0
  3.times do |index|
    total = total + index
  end
  (1..3).each do |value|
    total = total + value
  end
  values = [4, 5]
  values.each do |value|
    total = total + value
  end
end
"#,
    )
    .expect("times, range each, and array each should lower");
    let text = dump(&module);
    assert!(text.contains("for index in 0..3"));
    assert!(text.contains("for value in 1..=3"));
    assert!(text.contains("array_length(__pgl_each_values_"));
    assert!(text.contains("__pgl_each_values_"));
    assert!(text.contains("[__pgl_each_index_"));
    assert_eq!(text.matches("total = (total +").count(), 3);
}

#[test]
fn rejects_non_whitelisted_and_escaping_blocks_with_e0202() {
    for source in [
        "def setup\n  values = [1]\n  values.map { |value| value + 1 }\nend\n",
        "def setup\n  callback = proc { line(0, 0, 1, 1) }\nend\n",
        "def invoke(&callback)\n  callback.call\nend\n",
    ] {
        let diagnostics = lower(source).expect_err("general blocks must be rejected");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0202" && diagnostic.suggestion.is_some()
            })
        );
    }
}

#[test]
fn lowers_entry_control_flow_arithmetic_and_builtin_calls() {
    let module = lower(
        r#"
def setup
  x = 5 / 2
  remainder = -3 % 2
  x = x + 1
  if x
    background(0.0, 0.0, 0.0)
  end
  while x < 10
    x = x + 1
  end
end
"#,
    )
    .expect("supported Ruby should lower");
    let text = dump(&module);
    assert!(text.contains("entry setup() [host]"));
    assert!(text.contains("let x = (5 /int 2);"));
    assert!(text.contains("%floor"));
    assert!(text.contains("x = (x + 1);"));
    assert!(text.contains("if (not falsy?(x))"));
    assert!(text.contains("builtin#4(0.0, 0.0, 0.0);"));
    assert!(text.contains("while (not falsy?((x < 10)))"));
}

#[test]
fn lowers_top_level_constants_and_references() {
    let module = lower(
        r#"
SCALE = 1.5
MESH = mesh_box(SCALE, SCALE, SCALE)

def setup
  node_add(MESH, material_basic(vec4(1.0, 0.5, 0.25, 1.0)))
end
"#,
    )
    .expect("Common Core constants should lower");
    let text = dump(&module);
    assert!(text.contains("const SCALE = 1.5;"));
    assert!(text.contains("const MESH = builtin#26(SCALE, SCALE, SCALE);"));
    assert!(text.contains("builtin#31(MESH, builtin#30("));

    let diagnostics =
        lower("def setup\n  line(MISSING, 0, 1, 1)\nend\n").expect_err("unknown constants fail");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0200"
            && diagnostic.message.contains("constant is not declared")
            && diagnostic.suggestion.is_some()
    }));
}

#[test]
fn lowers_functions_parameters_implicit_returns_and_draw_alias() {
    let module = lower(
        r#"
def twice(value)
  value * 2
end

def draw(dt)
  circle(twice(dt), 20.0, 4.0)
end
"#,
    )
    .expect("supported Ruby should lower");
    let text = dump(&module);
    assert!(text.contains("fn twice(value) [auto]"));
    assert!(text.contains("return (value * 2);"));
    assert!(text.contains("entry frame(dt) [host]"));
    assert!(text.contains("builtin#9(twice(dt), 20.0, 4.0);"));
    assert!(matches!(
        &module.items[1],
        Item::Entry(entry) if entry.kind == EntryPointKind::Frame
    ));
}

#[test]
fn lowers_shader_entries_and_vector_constructors() {
    let module = lower(
        r#"
# @pgl position: vec3
def vertex_plasma(position)
  vec4(0.0, 0.0, 0.0, 1.0)
end

def fragment_plasma
  vec4(1.0, 0.5, 0.25, 1.0)
end
"#,
    )
    .expect("shader entries and vectors should lower");
    let text = dump(&module);
    assert!(text.contains("entry vertex_plasma(position: vec3) [gpu]"));
    assert!(text.contains("return vec4(0.0, 0.0, 0.0, 1.0);"));
    assert!(text.contains("entry fragment_plasma() [gpu]"));
    assert!(text.contains("return vec4(1.0, 0.5, 0.25, 1.0);"));
}

#[test]
fn lowers_builtin_event_field_reads() {
    let module = lower(
        r#"
def on_event(event)
  if event.kind == "pointerdown"
    line(event.x, event.y, mouse_x(), mouse_y())
  end
end
"#,
    )
    .expect("event fields should lower");
    let text = dump(&module);
    assert!(text.contains("entry on_event(event) [host]"));
    assert!(text.contains("event.kind"));
    assert!(text.contains("builtin#10(event.x, event.y"));

    let diagnostics = lower("def inspect(value)\n  value.length\nend\n")
        .expect_err("undeclared receiver methods are rejected");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not declared"))
    );
}

#[test]
fn preserves_implicit_returns_for_assignments_and_conditionals() {
    let module = lower(
        r#"
def assigned
  value = 1
end

def choose(flag)
  if flag
    2
  else
    3
  end
end

def empty
end

def empty_explicit
  return
end

def paint(x)
  circle(x, 0.0, 1.0)
end

def maybe(flag)
  if flag
    1
  end
end
"#,
    )
    .expect("supported Ruby should lower");
    let text = dump(&module);
    assert!(text.contains("let value = 1;\n    return value;"));
    assert!(text.contains("return 2;"));
    assert!(text.contains("return 3;"));
    assert!(text.contains("fn empty() [auto]\n  {\n    return;"));
    assert_eq!(text.matches("return;").count(), 2);
    assert!(text.contains("fn paint(x) [auto]"));
    assert!(text.contains("return builtin#9(x, 0.0, 1.0);"));
    assert!(text.contains("fn maybe(flag) [auto]"));
    assert!(text.contains("return none;"));
}

#[test]
fn preserves_ruby_negation_and_short_circuit_conditions() {
    let module = lower(
        r#"
def negated(value)
  !value
end

def choose(left, right)
  if left && !right
    1
  else
    0
  end
end
"#,
    )
    .expect("supported Ruby should lower");
    let text = dump(&module);
    assert!(text.contains("return falsy?(value);"));
    assert!(text.contains("if ((not falsy?(left)) and (not (not falsy?(right))))"));
}

#[test]
fn rejects_branch_locals_used_after_their_hir_scope() {
    let diagnostics = lower(
        r#"
def choose(flag)
  if flag
    value = 1
  else
    value = 2
  end
  value
end
"#,
    )
    .expect_err("branch-local Ruby variables need an outer initialization");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.starts_with("E02"))
        .expect("scope diagnostic");
    assert!(diagnostic.message.contains("not declared"));
    assert!(
        diagnostic
            .suggestion
            .as_ref()
            .is_some_and(|suggestion| suggestion.message.contains("before entering"))
    );
}

#[test]
fn rejects_loop_control_outside_a_loop() {
    for keyword in ["break", "next"] {
        let source = format!("def setup\n  {keyword}\nend\n");
        let diagnostics = lower(&source).expect_err("loop control requires an enclosing loop");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| matches!(diagnostic.code.as_str(), "E0100" | "E0200"))
        );
    }

    lower("def setup\n  while true\n    break\n  end\nend\n")
        .expect("loop control remains valid in an enclosing loop");
}

#[test]
fn accepts_the_signed_integer_lower_boundary() {
    let module = lower("def minimum\n  -2147483648\nend\n").expect("i32 minimum is valid");
    assert!(dump(&module).contains("return -2147483648;"));
}

#[test]
fn lowers_pgl_comments_to_positioned_hir_annotations() {
    let module = lower(
        r#"
# @pgl value: float
def scale(value)
  # @pgl amount: float
  amount = 1
  # @pgl items: float[]
  items = 1
  value
end
"#,
    )
    .expect("valid annotations should lower");
    let Item::Function(function) = &module.items[0] else {
        panic!("expected function");
    };
    assert!(matches!(
        function.params[0].ty.as_ref().map(|ty| &ty.kind),
        Some(polygl_hir::TypeKind::Float)
    ));
    let StmtKind::Let { ty, .. } = &function.body.statements[0].kind else {
        panic!("expected amount binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(polygl_hir::TypeKind::Float)
    ));
    let StmtKind::Let { ty, .. } = &function.body.statements[1].kind else {
        panic!("expected items binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(polygl_hir::TypeKind::Array(_))
    ));
}

#[test]
fn reports_invalid_and_unused_pgl_annotations() {
    let invalid =
        lower("# @pgl value: number\ndef setup\nend\n").expect_err("unknown type must fail");
    assert!(invalid.iter().any(|diagnostic| diagnostic.code == "E0314"));

    let unused =
        lower("# @pgl missing: float\ndef setup\nend\n").expect_err("unused target must fail");
    assert!(unused.iter().any(|diagnostic| {
        diagnostic.code == "E0314" && diagnostic.message.contains("does not match")
    }));

    let void = lower("# @pgl value: void\ndef setup\n  value = 1\nend\n")
        .expect_err("void is not a value");
    assert!(void.iter().any(|diagnostic| diagnostic.code == "E0314"));
}

#[test]
fn ignores_non_directive_and_inline_pgl_comments() {
    let module = lower(
        r#"
# @pglossary is ordinary documentation
def setup
  value = 1 # @pgl value: float
end
"#,
    )
    .expect("only standalone comments with an exact @pgl token are directives");
    let Item::Entry(entry) = &module.items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected value binding");
    };
    assert!(ty.is_none());
}

#[test]
fn does_not_leak_annotations_across_function_boundaries() {
    let diagnostics = lower(
        r#"
def first
  # @pgl value: float
  1
end

def second
  value = 2
end
"#,
    )
    .expect_err("a directive in another function must remain unused");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0314" && diagnostic.message.contains("does not match")
    }));
}

#[test]
fn turns_repeated_local_writes_into_assignments() {
    let module = lower("def setup\n  x = 1\n  x = 2\nend\n").expect("valid assignment");
    let Item::Entry(entry) = &module.items[0] else {
        panic!("setup should be an entry");
    };
    assert!(matches!(
        entry.body.statements[0].kind,
        StmtKind::Let { .. }
    ));
    assert!(matches!(
        entry.body.statements[1].kind,
        StmtKind::Assign { .. }
    ));
}

#[test]
fn lowers_bare_returns_as_void() {
    let module = lower("def setup\n  return\nend\n").expect("bare setup return is valid");
    let Item::Entry(entry) = &module.items[0] else {
        panic!("setup should be an entry");
    };
    assert!(matches!(
        entry.body.statements[0].kind,
        StmtKind::Return(None)
    ));

    let module = lower("def value\n  return\nend\n").expect("bare function return is void");
    let Item::Function(function) = &module.items[0] else {
        panic!("value should be a function");
    };
    assert!(matches!(
        function.body.statements[0].kind,
        StmtKind::Return(None)
    ));
}

#[test]
fn reports_prism_parse_errors() {
    let diagnostics = lower("def setup(\n").expect_err("invalid Ruby must fail");
    let diagnostic = diagnostics.iter().next().expect("one parse error");
    assert_eq!(diagnostic.code, "E0100");
}

#[test]
fn rejects_define_method_with_a_suggestion() {
    let diagnostics =
        lower("define_method(:setup) {}\n").expect_err("dynamic methods are unsupported");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.starts_with("E02"))
        .expect("Common Core diagnostic");
    assert!(
        diagnostic
            .suggestion
            .as_ref()
            .is_some_and(|suggestion| suggestion.message.contains("regular `def"))
    );
}

use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
use polygl_core::BuiltinTable;
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
            FeatureTag::TruthinessSugar
        ]
    );
}

#[test]
fn lowers_entry_control_flow_arithmetic_and_builtin_calls() {
    let module = lower(
        r#"
def setup
  x = 5 / 2
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
    assert!(text.contains("x = (x + 1);"));
    assert!(text.contains("if (not falsy?(x))"));
    assert!(text.contains("builtin#4(0.0, 0.0, 0.0);"));
    assert!(text.contains("while (not falsy?((x < 10)))"));
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
"#,
    )
    .expect("supported Ruby should lower");
    let text = dump(&module);
    assert!(text.contains("let value = 1;\n    return value;"));
    assert!(text.contains("return 2;"));
    assert!(text.contains("return 3;"));
    assert!(text.contains("fn empty() [auto]\n  {\n    return none;"));
    assert_eq!(text.matches("return none;").count(), 2);
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
fn accepts_the_signed_integer_lower_boundary() {
    let module = lower("def minimum\n  -2147483648\nend\n").expect("i32 minimum is valid");
    assert!(dump(&module).contains("return -2147483648;"));
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

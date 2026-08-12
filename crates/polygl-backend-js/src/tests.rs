use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use polygl_builtins::RuntimeOp;
use polygl_lir::{
    BinaryOp, Block, CallTarget, Constant, Domain, EntryKind, EntryPoint, Expr, ExprKind,
    FieldInit, Function, Literal, MapEntry, Module, Parameter, Place, PlaceKind, Range, Statement,
    StatementKind,
};
use polygl_span::{SourceFile, SourceId, Span};
use polygl_types::Type;

use crate::{BuildMode, EmitError, JavaScriptBackend, SourceMapMode};

fn source() -> SourceFile {
    SourceFile::new(
        SourceId::new(7),
        "main.rb",
        "ANSWER = 41\ndef double(x)\n  x * 2\nend\ndef setup\n  values = [1, 2]\nend\n",
    )
}

fn span(source: &SourceFile, start: usize, end: usize) -> Span {
    source.span(start, end).unwrap()
}

fn expression(kind: ExprKind, ty: Type, span: Span) -> Expr {
    Expr::new(kind, ty, span)
}

fn int(value: i32, span: Span) -> Expr {
    expression(ExprKind::Literal(Literal::Int(value)), Type::Int, span)
}

fn float(value: f64, span: Span) -> Expr {
    expression(ExprKind::Literal(Literal::Float(value)), Type::Float, span)
}

fn string(value: &str, span: Span) -> Expr {
    expression(
        ExprKind::Literal(Literal::Str(value.to_owned())),
        Type::Str,
        span,
    )
}

fn variable(name: &str, ty: Type, span: Span) -> Expr {
    expression(ExprKind::Variable(name.to_owned()), ty, span)
}

fn block(statements: Vec<Statement>, span: Span) -> Block {
    Block { statements, span }
}

fn statement(kind: StatementKind, span: Span) -> Statement {
    Statement::new(kind, span)
}

fn sample_module(source: &SourceFile) -> Module {
    let module_span = span(source, 0, source.len());
    let constant_span = span(source, 0, 11);
    let function_span = span(source, 12, 37);
    let setup_span = span(source, 38, source.len());
    let multiply = expression(
        ExprKind::Binary {
            op: BinaryOp::Multiply,
            left: Box::new(variable("x", Type::Int, function_span)),
            right: Box::new(int(2, function_span)),
        },
        Type::Int,
        function_span,
    );
    let function = Function {
        name: "__pgl_6_double__int".to_owned(),
        params: vec![
            Parameter {
                name: "x".to_owned(),
                ty: Type::Int,
                span: function_span,
            },
            Parameter {
                name: "x$1".to_owned(),
                ty: Type::Int,
                span: function_span,
            },
        ],
        result: Type::Int,
        body: block(
            vec![
                statement(
                    StatementKind::Let {
                        name: "before_shadow".to_owned(),
                        ty: Type::Int,
                        init: variable("x", Type::Int, function_span),
                    },
                    function_span,
                ),
                statement(
                    StatementKind::Let {
                        name: "dollar_parameter".to_owned(),
                        ty: Type::Int,
                        init: variable("x$1", Type::Int, function_span),
                    },
                    function_span,
                ),
                statement(
                    StatementKind::Let {
                        name: "x".to_owned(),
                        ty: Type::Int,
                        init: int(3, function_span),
                    },
                    function_span,
                ),
                statement(StatementKind::Return(Some(multiply)), function_span),
            ],
            function_span,
        ),
        domain: Domain::Host,
        span: function_span,
    };
    let values = expression(
        ExprKind::Array(vec![int(1, setup_span), int(2, setup_span)]),
        Type::Array(Box::new(Type::Int)),
        setup_span,
    );
    let call = expression(
        ExprKind::Call {
            target: CallTarget::Function("__pgl_6_double__int".to_owned()),
            args: vec![
                expression(
                    ExprKind::Constant("ANSWER".to_owned()),
                    Type::Int,
                    setup_span,
                ),
                int(7, setup_span),
            ],
        },
        Type::Int,
        setup_span,
    );
    let runtime_call = expression(
        ExprKind::Call {
            target: CallTarget::Runtime(RuntimeOp::new("circle")),
            args: vec![
                float(10.0, setup_span),
                float(20.0, setup_span),
                float(5.0, setup_span),
            ],
        },
        Type::Unit,
        setup_span,
    );
    let entry = EntryPoint {
        kind: EntryKind::Setup,
        params: Vec::new(),
        result: Type::Unit,
        body: block(
            vec![
                statement(
                    StatementKind::Let {
                        name: "class".to_owned(),
                        ty: Type::Array(Box::new(Type::Int)),
                        init: values,
                    },
                    setup_span,
                ),
                statement(
                    StatementKind::Let {
                        name: "result".to_owned(),
                        ty: Type::Int,
                        init: call,
                    },
                    setup_span,
                ),
                statement(StatementKind::Expr(runtime_call), setup_span),
                statement(
                    StatementKind::Let {
                        name: "remainder".to_owned(),
                        ty: Type::Int,
                        init: expression(
                            ExprKind::Binary {
                                op: BinaryOp::FloorRemainder,
                                left: Box::new(int(-3, setup_span)),
                                right: Box::new(int(2, setup_span)),
                            },
                            Type::Int,
                            setup_span,
                        ),
                    },
                    setup_span,
                ),
                statement(
                    StatementKind::For {
                        variable: "i".to_owned(),
                        range: Range {
                            start: int(0, setup_span),
                            end: int(i32::MAX, setup_span),
                            inclusive: true,
                            span: setup_span,
                        },
                        body: block(
                            vec![
                                statement(
                                    StatementKind::Let {
                                        name: "i".to_owned(),
                                        ty: Type::Int,
                                        init: int(9, setup_span),
                                    },
                                    setup_span,
                                ),
                                statement(StatementKind::Continue, setup_span),
                            ],
                            setup_span,
                        ),
                    },
                    setup_span,
                ),
            ],
            setup_span,
        ),
        domain: Domain::Host,
        span: setup_span,
    };
    Module {
        functions: vec![function],
        structs: Vec::new(),
        constants: vec![Constant {
            name: "ANSWER".to_owned(),
            ty: Type::Int,
            value: int(41, constant_span),
            domain: Domain::Host,
            span: constant_span,
        }],
        entries: vec![entry],
        span: module_span,
    }
}

#[test]
fn emits_es2020_runtime_calls_wrapping_ints_and_source_map_v3() {
    let source = source();
    let artifacts = JavaScriptBackend::new(BuildMode::Debug)
        .generate(&sample_module(&source), std::slice::from_ref(&source))
        .unwrap();

    assert!(
        artifacts
            .javascript
            .starts_with("import * as __pglRuntime from \"./runtime.js\";")
    );
    assert!(
        artifacts
            .javascript
            .contains("export const __polyglRuntimeAbi = 2;")
    );
    assert!(artifacts.javascript.contains("let before_shadow = x;"));
    assert!(artifacts.javascript.contains("let dollar_parameter = x$1;"));
    assert!(artifacts.javascript.contains("Math.imul(x$2, 2)"));
    assert!(
        artifacts
            .javascript
            .contains("__pglIntFloorRemainder(-3, 2, __pglSpans[")
    );
    assert!(artifacts.javascript.contains("__pglRuntime.circle("));
    assert!(artifacts.javascript.contains("export function setup()"));
    assert!(artifacts.javascript.contains("__pglLocal_636c617373"));
    assert!(artifacts.javascript.contains("__pglRangeDone0"));
    assert!(
        artifacts
            .javascript
            .contains("__pglRangeIndex0 === __pglRangeEnd0")
    );
    assert!(artifacts.javascript.contains("let i = __pglRangeIndex0;"));
    assert!(
        artifacts
            .javascript
            .contains(
                "(x, x$1) {\n  {\n    let before_shadow = x;\n    let dollar_parameter = x$1;\n    let x$2 = 3;"
            )
    );
    assert!(
        artifacts
            .javascript
            .contains("let i = __pglRangeIndex0;\n        {\n          let i$1 = 9;")
    );
    assert!(
        artifacts
            .javascript
            .ends_with("//# sourceMappingURL=app.js.map\n")
    );

    let map: serde_json::Value =
        serde_json::from_str(artifacts.source_map.as_deref().unwrap()).unwrap();
    assert_eq!(map["version"], 3);
    assert_eq!(map["file"], "app.js");
    assert_eq!(map["sources"][0], "main.rb");
    assert_eq!(map["sourcesContent"][0], source.text());
    assert!(!map["mappings"].as_str().unwrap().is_empty());
}

#[test]
fn inserts_debug_index_and_nil_checks_and_removes_them_in_release() {
    let source = source();
    let check_span = span(&source, 52, source.len());
    let array_type = Type::Array(Box::new(Type::Int));
    let values = variable("values", array_type.clone(), check_span);
    let record = variable("record", Type::Map(Box::new(Type::Int)), check_span);
    let module = Module {
        functions: Vec::new(),
        structs: Vec::new(),
        constants: Vec::new(),
        entries: vec![EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: block(
                vec![
                    statement(
                        StatementKind::Let {
                            name: "read".to_owned(),
                            ty: Type::Int,
                            init: expression(
                                ExprKind::Index {
                                    base: Box::new(values.clone()),
                                    index: Box::new(int(0, check_span)),
                                },
                                Type::Int,
                                check_span,
                            ),
                        },
                        check_span,
                    ),
                    statement(
                        StatementKind::Assign {
                            target: Place {
                                kind: PlaceKind::Index {
                                    base: values,
                                    index: int(1, check_span),
                                },
                                span: check_span,
                            },
                            value: int(3, check_span),
                        },
                        check_span,
                    ),
                    statement(
                        StatementKind::Let {
                            name: "mapped".to_owned(),
                            ty: Type::Int,
                            init: expression(
                                ExprKind::Index {
                                    base: Box::new(record.clone()),
                                    index: Box::new(string("value", check_span)),
                                },
                                Type::Int,
                                check_span,
                            ),
                        },
                        check_span,
                    ),
                    statement(
                        StatementKind::Assign {
                            target: Place {
                                kind: PlaceKind::Index {
                                    base: record.clone(),
                                    index: string("value", check_span),
                                },
                                span: check_span,
                            },
                            value: int(4, check_span),
                        },
                        check_span,
                    ),
                    statement(
                        StatementKind::Let {
                            name: "field".to_owned(),
                            ty: Type::Int,
                            init: expression(
                                ExprKind::Field {
                                    base: Box::new(record.clone()),
                                    field: "value".to_owned(),
                                },
                                Type::Int,
                                check_span,
                            ),
                        },
                        check_span,
                    ),
                    statement(
                        StatementKind::Assign {
                            target: Place {
                                kind: PlaceKind::Field {
                                    base: record,
                                    field: "value".to_owned(),
                                },
                                span: check_span,
                            },
                            value: int(4, check_span),
                        },
                        check_span,
                    ),
                ],
                check_span,
            ),
            domain: Domain::Host,
            span: check_span,
        }],
        span: check_span,
    };

    let debug = JavaScriptBackend::new(BuildMode::Debug)
        .generate(&module, std::slice::from_ref(&source))
        .unwrap()
        .javascript;
    assert!(debug.contains("const __pglSpans = Object.freeze(["));
    assert!(debug.contains("\"source\":\"main.rb\""));
    assert!(debug.contains("__pglRuntime.checkedIndex("));
    assert!(debug.contains("__pglRuntime.checkIndex("));
    assert!(debug.contains("__pglRuntime.mapGet(record, \"value\", __pglSpans["));
    assert!(debug.contains("__pglRuntime.mapSet(record, \"value\", 4, __pglSpans["));
    assert!(debug.contains("__pglRuntime.requireNonNil("));

    let release = JavaScriptBackend::new(BuildMode::Release)
        .generate(&module, std::slice::from_ref(&source))
        .unwrap()
        .javascript;
    assert!(!release.contains("__pglSpans"));
    assert!(!release.contains("checkedIndex"));
    assert!(!release.contains("requireNonNil"));
    assert!(release.contains("(values)[0]"));
    assert!(release.contains("__pglRuntime.mapGet(record, \"value\")"));
    assert!(release.contains("__pglRuntime.mapSet(record, \"value\", 4)"));
}

#[test]
fn emits_safe_records_for_map_and_struct_literals() {
    let source = source();
    let literal_span = span(&source, 38, source.len());
    let entries = ["__proto__", "constructor", "toString", "", "日本語"]
        .into_iter()
        .enumerate()
        .map(|(index, key)| MapEntry {
            key: string(key, literal_span),
            value: int(i32::try_from(index).unwrap(), literal_span),
            span: literal_span,
        })
        .collect();
    let module = Module {
        functions: Vec::new(),
        structs: Vec::new(),
        constants: Vec::new(),
        entries: vec![EntryPoint {
            kind: EntryKind::Setup,
            params: Vec::new(),
            result: Type::Unit,
            body: block(
                vec![
                    statement(
                        StatementKind::Expr(expression(
                            ExprKind::Map(entries),
                            Type::Map(Box::new(Type::Int)),
                            literal_span,
                        )),
                        literal_span,
                    ),
                    statement(
                        StatementKind::Expr(expression(
                            ExprKind::Struct {
                                name: "SpecialFields".to_owned(),
                                fields: vec![
                                    FieldInit {
                                        name: "__proto__".to_owned(),
                                        value: int(1, literal_span),
                                        span: literal_span,
                                    },
                                    FieldInit {
                                        name: "constructor".to_owned(),
                                        value: int(2, literal_span),
                                        span: literal_span,
                                    },
                                ],
                            },
                            Type::Unit,
                            literal_span,
                        )),
                        literal_span,
                    ),
                ],
                literal_span,
            ),
            domain: Domain::Host,
            span: literal_span,
        }],
        span: literal_span,
    };

    let javascript = JavaScriptBackend::default()
        .generate(&module, std::slice::from_ref(&source))
        .unwrap()
        .javascript;
    assert!(javascript.contains(
        "__pglRuntime.mapFromEntries([[\"__proto__\", 0], [\"constructor\", 1], [\"toString\", 2], [\"\", 3], [\"日本語\", 4]])"
    ));
    assert!(
        javascript
            .contains("__pglRuntime.structFromEntries([[\"__proto__\", 1], [\"constructor\", 2]])")
    );
    assert!(!javascript.contains("Object.fromEntries"));
}

#[test]
fn reports_missing_and_duplicate_source_ids() {
    let source = source();
    let module = sample_module(&source);
    assert_eq!(
        JavaScriptBackend::default().generate(&module, &[]),
        Err(EmitError::MissingSource(SourceId::new(7)))
    );
    assert_eq!(
        JavaScriptBackend::default().generate(&module, &[source.clone(), source]),
        Err(EmitError::DuplicateSource(SourceId::new(7)))
    );
}

#[test]
fn configures_external_inline_and_omitted_source_maps() {
    let source = source();
    let module = sample_module(&source);

    let none = JavaScriptBackend::default()
        .with_source_map_mode(SourceMapMode::None)
        .generate(&module, std::slice::from_ref(&source))
        .unwrap();
    assert!(none.source_map.is_none());
    assert!(!none.javascript.contains("sourceMappingURL"));

    let external = JavaScriptBackend::default()
        .with_sources_content(false)
        .generate(&module, std::slice::from_ref(&source))
        .unwrap();
    let external_map: serde_json::Value =
        serde_json::from_str(external.source_map.as_deref().unwrap()).unwrap();
    assert!(
        external
            .javascript
            .ends_with("sourceMappingURL=app.js.map\n")
    );
    assert!(external_map.get("sourcesContent").is_none());

    let inline = JavaScriptBackend::default()
        .with_source_map_mode(SourceMapMode::Inline)
        .with_sources_content(false)
        .generate(&module, std::slice::from_ref(&source))
        .unwrap();
    assert!(inline.source_map.is_none());
    let encoded = inline
        .javascript
        .trim_end()
        .rsplit_once("base64,")
        .unwrap()
        .1;
    let decoded = STANDARD.decode(encoded).unwrap();
    let inline_map: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(inline_map["file"], "app.js");
    assert!(inline_map.get("sourcesContent").is_none());
}

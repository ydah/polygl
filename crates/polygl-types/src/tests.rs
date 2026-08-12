use polygl_hir::{
    BinOp, Block, Callee, ConstDef, EntryPoint, EntryPointKind, Expr, ExprKind, Function,
    HirBuilder, Item, Literal, Module, Param, Place, PlaceKind, Stmt, StmtKind, StructDef, Symbol,
    TypeExpr, TypeKind, UnOp,
};
use polygl_span::{SourceFile, SourceId, Span};

use crate::{Type, analyze};

fn span() -> Span {
    SourceFile::new(SourceId::new(1), "test.rb", "x")
        .span(0, 1)
        .unwrap()
}

fn expression(kind: ExprKind) -> Expr {
    Expr::new(kind, span())
}

fn parameter(name: &str) -> Param {
    Param {
        name: Symbol::new(name),
        ty: None,
        span: span(),
    }
}

fn function(name: &str, params: &[&str], statements: Vec<Stmt>) -> Item {
    Item::Function(Function {
        name: Symbol::new(name),
        params: params.iter().map(|name| parameter(name)).collect(),
        return_type: None,
        body: Block {
            statements,
            span: span(),
        },
        span: span(),
        domain: polygl_hir::DomainHint::Auto,
    })
}

fn setup(statements: Vec<Stmt>) -> Item {
    Item::Entry(EntryPoint {
        kind: EntryPointKind::Setup,
        params: Vec::new(),
        return_type: None,
        body: Block {
            statements,
            span: span(),
        },
        span: span(),
    })
}

fn module(items: Vec<Item>) -> Module {
    Module {
        items,
        span: span(),
    }
}

fn call(name: &str, args: Vec<Expr>) -> Expr {
    expression(ExprKind::Call {
        callee: Callee::User(Symbol::new(name)),
        args,
    })
}

fn builtin(id: polygl_hir::BuiltinId, args: Vec<Expr>) -> Expr {
    expression(ExprKind::Call {
        callee: Callee::Builtin(id),
        args,
    })
}

fn return_value(value: Expr) -> Stmt {
    Stmt::new(StmtKind::Return(Some(value)), span())
}

#[test]
fn monomorphizes_calls_and_rewrites_type_dependent_operators() {
    let builder = HirBuilder::new(span());
    let divide = expression(ExprKind::Binary {
        op: BinOp::DivInt,
        left: Box::new(builder.variable("value")),
        right: Box::new(builder.int(2)),
    });
    let concatenate = expression(ExprKind::Binary {
        op: BinOp::Add,
        left: Box::new(builder.string("a")),
        right: Box::new(builder.string("b")),
    });
    let hir = module(vec![
        function("half", &["value"], vec![return_value(divide)]),
        setup(vec![
            builder.expression(call("half", vec![builder.int(4)])),
            builder.expression(call("half", vec![builder.float(4.0)])),
            builder.let_value("label", concatenate),
        ]),
    ]);

    let typed = analyze(&hir).expect("types should resolve");
    assert_eq!(typed.instance_count("half"), 2);
    let functions = typed
        .as_hir()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);
    assert!(functions.iter().any(|function| {
        function.name.as_str() == "__pgl_4_half__int"
            && matches!(
                function.body.statements[0].kind,
                StmtKind::Return(Some(Expr {
                    kind: ExprKind::Binary {
                        op: BinOp::DivInt,
                        ..
                    },
                    ..
                }))
            )
    }));
    assert!(functions.iter().any(|function| {
        function.name.as_str() == "__pgl_4_half__float"
            && matches!(
                function.body.statements[0].kind,
                StmtKind::Return(Some(Expr {
                    kind: ExprKind::Binary {
                        op: BinOp::DivFloat,
                        ..
                    },
                    ..
                }))
            )
    }));
    let Item::Entry(entry) = typed.as_hir().items.last().unwrap() else {
        panic!("last item should be setup");
    };
    let StmtKind::Let { ty, init, .. } = &entry.body.statements[2].kind else {
        panic!("label should be a binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Str)
    ));
    assert!(matches!(
        init.kind,
        ExprKind::Binary {
            op: BinOp::StrConcat,
            ..
        }
    ));

    let contextual_add = expression(ExprKind::Binary {
        op: BinOp::Add,
        left: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("left")),
            index: Box::new(builder.int(0)),
        })),
        right: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("right")),
            index: Box::new(builder.int(0)),
        })),
    });
    let typed = analyze(&module(vec![setup(vec![
        builder.let_value("left", expression(ExprKind::Array(Vec::new()))),
        builder.let_value("right", expression(ExprKind::Array(Vec::new()))),
        builder.expression(builtin(
            polygl_hir::BuiltinId::TEXT,
            vec![contextual_add, builder.float(0.0), builder.float(0.0)],
        )),
    ])]))
    .expect("builtin result context should infer both string operands");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    for index in 0..2 {
        let StmtKind::Let { ty, .. } = &entry.body.statements[index].kind else {
            panic!("expected array binding");
        };
        assert!(matches!(
            ty.as_ref().map(|ty| &ty.kind),
            Some(TypeKind::Array(element)) if matches!(element.kind, TypeKind::Str)
        ));
    }
}

#[test]
fn normalizes_annotated_parameters_before_selecting_an_instance() {
    let builder = HirBuilder::new(span());
    let mut annotated = function(
        "measure",
        &["value"],
        vec![return_value(builder.variable("value"))],
    );
    let Item::Function(function) = &mut annotated else {
        unreachable!("helper always returns a function");
    };
    function.params[0].ty = Some(TypeExpr::new(TypeKind::Float, span()));
    let hir = module(vec![
        annotated,
        setup(vec![
            builder.expression(call("measure", vec![builder.int(1)])),
            builder.expression(call("measure", vec![builder.float(2.0)])),
        ]),
    ]);

    let typed = analyze(&hir).expect("int should normalize to the annotated float type");
    assert_eq!(typed.instance_count("measure"), 1);
    let function = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("one specialized function");
    assert_eq!(function.name.as_str(), "__pgl_7_measure__float");
    assert!(matches!(
        function.params[0].ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
}

#[test]
fn builtin_constraints_flow_back_to_local_bindings() {
    let builder = HirBuilder::new(span());
    let hir = module(vec![setup(vec![
        builder.let_value("x", builder.int(1)),
        builder.expression(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![builder.variable("x"), builder.int(2), builder.int(3)],
        )),
    ])]);
    let typed = analyze(&hir).expect("int should widen to the builtin float constraint");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
}

#[test]
fn tier_two_builtins_keep_handles_distinct_and_constrain_uniform_values() {
    let builder = HirBuilder::new(span());
    let color = expression(ExprKind::Vector {
        size: 4,
        args: vec![
            builder.float(0.2),
            builder.float(0.4),
            builder.float(0.6),
            builder.float(1.0),
        ],
    });
    let valid = module(vec![setup(vec![
        builder.let_value(
            "mesh",
            builtin(
                polygl_hir::BuiltinId::MESH_BOX,
                vec![builder.float(1.0), builder.float(2.0), builder.float(3.0)],
            ),
        ),
        builder.let_value(
            "material",
            builtin(polygl_hir::BuiltinId::MATERIAL_BASIC, vec![color]),
        ),
        builder.let_value(
            "node",
            builtin(
                polygl_hir::BuiltinId::NODE_ADD,
                vec![builder.variable("mesh"), builder.variable("material")],
            ),
        ),
        builder.expression(builtin(
            polygl_hir::BuiltinId::SHADER_SET,
            vec![
                builder.variable("node"),
                builder.string("roughness"),
                builder.float(0.5),
            ],
        )),
    ])]);
    let typed = analyze(&valid).expect("Tier 2 handles and a float uniform should type-check");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let expected = [
        TypeKind::Opaque(polygl_hir::OpaqueType::Mesh),
        TypeKind::Opaque(polygl_hir::OpaqueType::Material),
        TypeKind::Opaque(polygl_hir::OpaqueType::Node),
    ];
    for (statement, expected) in entry.body.statements.iter().zip(expected) {
        let StmtKind::Let { ty: Some(ty), .. } = &statement.kind else {
            panic!("expected typed Tier 2 binding");
        };
        assert_eq!(ty.kind, expected);
    }

    let invalid_handle = module(vec![setup(vec![builder.expression(builtin(
        polygl_hir::BuiltinId::NODE_ADD,
        vec![builder.int(1), builder.int(2)],
    ))])]);
    let diagnostics = analyze(&invalid_handle).expect_err("integers are not opaque handles");
    assert!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0303")
            .count()
            >= 2
    );

    let invalid_uniform = module(vec![setup(vec![
        builder.let_value(
            "mesh",
            builtin(
                polygl_hir::BuiltinId::MESH_BOX,
                vec![builder.float(1.0), builder.float(1.0), builder.float(1.0)],
            ),
        ),
        builder.let_value(
            "material",
            builtin(
                polygl_hir::BuiltinId::MATERIAL_SHADER,
                vec![builder.string("main")],
            ),
        ),
        builder.let_value(
            "node",
            builtin(
                polygl_hir::BuiltinId::NODE_ADD,
                vec![builder.variable("mesh"), builder.variable("material")],
            ),
        ),
        builder.expression(builtin(
            polygl_hir::BuiltinId::SHADER_SET,
            vec![
                builder.variable("node"),
                builder.string("label"),
                builder.string("not a shader value"),
            ],
        )),
    ])]);
    let diagnostics = analyze(&invalid_uniform).expect_err("strings are not shader values");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0303")
    );
}

#[test]
fn opaque_handle_names_are_reserved_for_runtime_types() {
    let definition = Item::Struct(StructDef {
        name: Symbol::new("Mesh"),
        fields: Vec::new(),
        methods: Vec::new(),
        span: span(),
    });
    let diagnostics =
        analyze(&module(vec![definition, setup(Vec::new())])).expect_err("Mesh is reserved");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0306")
    );
}

#[test]
fn specializes_after_backward_constraints_stabilize() {
    let builder = HirBuilder::new(span());
    let consume = function(
        "consume",
        &["value"],
        vec![return_value(builder.variable("value"))],
    );
    let hir = module(vec![
        consume,
        setup(vec![
            builder.let_value("x", builder.int(1)),
            builder.expression(call(
                "consume",
                vec![expression(ExprKind::Binary {
                    op: BinOp::DivInt,
                    left: Box::new(builder.variable("x")),
                    right: Box::new(builder.int(2)),
                })],
            )),
            builder.expression(builtin(
                polygl_hir::BuiltinId::CIRCLE,
                vec![
                    builder.variable("x"),
                    builder.float(2.0),
                    builder.float(3.0),
                ],
            )),
        ]),
    ]);

    let typed = analyze(&hir).expect("the later builtin constraint should select float");
    assert_eq!(typed.instance_count("consume"), 1);
    let function = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("one specialized function");
    assert_eq!(function.name.as_str(), "__pgl_7_consume__float");
    let Item::Entry(entry) = typed.as_hir().items.last().unwrap() else {
        panic!("expected setup");
    };
    let StmtKind::Expr(Expr {
        kind: ExprKind::Call {
            callee: Callee::User(callee),
            args,
        },
        ..
    }) = &entry.body.statements[1].kind
    else {
        panic!("expected user call");
    };
    assert_eq!(callee.as_str(), "__pgl_7_consume__float");
    assert!(matches!(
        args[0].ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    assert!(matches!(
        args[0].kind,
        ExprKind::Binary {
            op: BinOp::DivFloat,
            ..
        }
    ));
}

#[test]
fn annotated_parameters_constrain_empty_aggregate_arguments() {
    let builder = HirBuilder::new(span());
    let mut accept = function(
        "accept",
        &["items"],
        vec![return_value(builder.variable("items"))],
    );
    let Item::Function(function) = &mut accept else {
        unreachable!("helper always returns a function");
    };
    function.params[0].ty = Some(TypeExpr::new(
        TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Int, span()))),
        span(),
    ));
    let typed = analyze(&module(vec![
        accept,
        setup(vec![builder.expression(call(
            "accept",
            vec![expression(ExprKind::Array(Vec::new()))],
        ))]),
    ]))
    .expect("the parameter annotation should flow into the empty array");
    let Item::Entry(entry) = typed.as_hir().items.last().unwrap() else {
        panic!("expected setup");
    };
    let StmtKind::Expr(Expr {
        kind: ExprKind::Call { args, .. },
        ..
    }) = &entry.body.statements[0].kind
    else {
        panic!("expected call");
    };
    assert!(matches!(
        args[0].ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Array(element)) if matches!(element.kind, TypeKind::Int)
    ));
}

#[test]
fn permits_only_int_to_float_reassignment() {
    let builder = HirBuilder::new(span());
    let target = Place {
        kind: PlaceKind::Var(Symbol::new("value")),
        span: span(),
    };
    let widening = module(vec![setup(vec![
        builder.let_value("value", builder.int(1)),
        Stmt::new(
            StmtKind::Assign {
                target: target.clone(),
                value: builder.float(2.0),
            },
            span(),
        ),
    ])]);
    let typed = analyze(&widening).expect("int to float is the one widening");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));

    let invalid = module(vec![setup(vec![
        builder.let_value("value", builder.bool(true)),
        Stmt::new(
            StmtKind::Assign {
                target,
                value: builder.int(2),
            },
            span(),
        ),
    ])]);
    let diagnostics = analyze(&invalid).expect_err("bool to int must fail");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0311")
    );

    let aggregate = module(vec![setup(vec![
        builder.let_value("items", expression(ExprKind::Array(vec![builder.int(1)]))),
        Stmt::new(
            StmtKind::Assign {
                target: Place {
                    kind: PlaceKind::Var(Symbol::new("items")),
                    span: span(),
                },
                value: expression(ExprKind::Array(vec![builder.float(2.0)])),
            },
            span(),
        ),
    ])]);
    let diagnostics = analyze(&aggregate).expect_err("aggregate element types remain fixed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0311")
    );
}

#[test]
fn revalidates_assignments_after_later_constraints() {
    let builder = HirBuilder::new(span());
    let assignment = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Var(Symbol::new("result")),
                span: span(),
            },
            value: expression(ExprKind::Binary {
                op: BinOp::DivInt,
                left: Box::new(builder.variable("source")),
                right: Box::new(builder.int(2)),
            }),
        },
        span(),
    );
    let typed = analyze(&module(vec![setup(vec![
        builder.let_value("source", builder.int(1)),
        builder.let_value("result", builder.int(1)),
        assignment,
        builder.expression(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![
                builder.variable("source"),
                builder.float(0.0),
                builder.float(1.0),
            ],
        )),
    ])]))
    .expect("the assignment target should widen with its final right-hand side");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[1].kind else {
        panic!("expected result binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    let StmtKind::Assign { value, .. } = &entry.body.statements[2].kind else {
        panic!("expected assignment");
    };
    assert!(matches!(
        value.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
}

#[test]
fn keeps_annotations_and_parameter_bindings_invariant() {
    let builder = HirBuilder::new(span());
    let annotated = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("value"),
            ty: Some(TypeExpr::new(TypeKind::Int, span())),
            init: builder.int(1),
        },
        span(),
    );
    let reassign = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Var(Symbol::new("value")),
                span: span(),
            },
            value: builder.float(2.0),
        },
        span(),
    );
    assert!(
        analyze(&module(vec![setup(vec![annotated, reassign])])).is_err(),
        "an explicit int annotation must not be widened"
    );

    let contextual = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("fixed"),
            ty: Some(TypeExpr::new(TypeKind::Int, span())),
            init: builder.int(1),
        },
        span(),
    );
    let typed = analyze(&module(vec![setup(vec![
        contextual,
        builder.expression(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![
                builder.variable("fixed"),
                builder.float(0.0),
                builder.float(1.0),
            ],
        )),
    ])]))
    .expect("int-to-float argument coercion must not mutate an annotated binding");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected fixed binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Int)
    ));

    let aggregate = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("items"),
            ty: Some(TypeExpr::new(
                TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Float, span()))),
                span(),
            )),
            init: expression(ExprKind::Array(vec![builder.int(1)])),
        },
        span(),
    );
    assert!(
        analyze(&module(vec![setup(vec![aggregate])])).is_err(),
        "container annotations must not widen their element type"
    );

    let parameter_write = function(
        "change",
        &["parameter"],
        vec![
            Stmt::new(
                StmtKind::Assign {
                    target: Place {
                        kind: PlaceKind::Var(Symbol::new("parameter")),
                        span: span(),
                    },
                    value: builder.float(2.0),
                },
                span(),
            ),
            return_value(builder.variable("parameter")),
        ],
    );
    let typed = analyze(&module(vec![
        parameter_write,
        setup(vec![
            builder.let_value("caller", builder.int(1)),
            builder.expression(call("change", vec![builder.variable("caller")])),
            builder.expression(builtin(
                polygl_hir::BuiltinId::SIZE,
                vec![builder.variable("caller"), builder.variable("caller")],
            )),
        ]),
    ]))
    .expect("parameter widening is local and must not alias the caller");
    let function = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("specialized function");
    assert!(matches!(
        function.params[0].ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    let Item::Entry(entry) = typed.as_hir().items.last().unwrap() else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected caller binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Int)
    ));

    let frame = Item::Entry(EntryPoint {
        kind: EntryPointKind::Frame,
        params: vec![Param {
            name: Symbol::new("dt"),
            ty: Some(TypeExpr::new(TypeKind::Int, span())),
            span: span(),
        }],
        return_type: None,
        body: Block {
            statements: Vec::new(),
            span: span(),
        },
        span: span(),
    });
    assert!(
        analyze(&module(vec![frame])).is_err(),
        "fixed entry ABI annotations must be validated"
    );
}

#[test]
fn normalizes_instances_after_body_constraints_without_aliasing_callers() {
    let builder = HirBuilder::new(span());
    let constrained = function(
        "constrained",
        &["parameter"],
        vec![
            builder.expression(builtin(
                polygl_hir::BuiltinId::CIRCLE,
                vec![
                    builder.variable("parameter"),
                    builder.float(0.0),
                    builder.float(1.0),
                ],
            )),
            return_value(builder.variable("parameter")),
        ],
    );
    let typed = analyze(&module(vec![
        constrained,
        setup(vec![
            builder.let_value("caller", builder.int(1)),
            builder.expression(call("constrained", vec![builder.variable("caller")])),
            builder.expression(call("constrained", vec![builder.float(2.0)])),
        ]),
    ]))
    .expect("body constraints should normalize both calls to one float instance");

    assert_eq!(typed.instance_count("constrained"), 1);
    let function = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("specialized function");
    assert!(function.name.as_str().ends_with("__float"));
    assert!(matches!(
        function.params[0].ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    let Item::Entry(entry) = typed.as_hir().items.last().unwrap() else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected caller binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Int)
    ));
}

#[test]
fn infers_option_values_through_nil_checks_and_reassignment() {
    let builder = HirBuilder::new(span());
    let assign = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Var(Symbol::new("value")),
                span: span(),
            },
            value: builder.int(1),
        },
        span(),
    );
    let typed = analyze(&module(vec![setup(vec![
        builder.let_value("value", expression(ExprKind::Literal(Literal::None))),
        builder.expression(expression(ExprKind::NilCheck(Box::new(
            builder.variable("value"),
        )))),
        assign,
    ])]))
    .expect("nil check and later value assignment should infer Option<int>");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, .. } = &entry.body.statements[0].kind else {
        panic!("expected option binding");
    };
    assert!(matches!(
        ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Option(inner)) if matches!(inner.kind, TypeKind::Int)
    ));

    let invalid = module(vec![setup(vec![
        builder.let_value("value", builder.int(1)),
        builder.expression(expression(ExprKind::NilCheck(Box::new(
            builder.variable("value"),
        )))),
    ])]);
    assert!(
        analyze(&invalid).is_err(),
        "nil checks must not implicitly turn concrete values into options"
    );
}

#[test]
fn rejects_void_in_value_positions() {
    let builder = HirBuilder::new(span());
    let void_call = || builtin(polygl_hir::BuiltinId::NO_STROKE, Vec::new());
    let standalone = analyze(&module(vec![setup(vec![builder.expression(void_call())])]));
    assert!(standalone.is_ok(), "void calls are valid statements");

    let binding = module(vec![setup(vec![builder.let_value("invalid", void_call())])]);
    assert!(analyze(&binding).is_err());

    let truthiness =
        module(vec![setup(vec![builder.expression(expression(
            ExprKind::FalsyCheck(Box::new(void_call())),
        ))])]);
    assert!(
        analyze(&truthiness).is_err(),
        "truthiness checks require a value operand"
    );

    let constant = module(vec![
        Item::Const(ConstDef {
            name: Symbol::new("INVALID"),
            ty: None,
            value: void_call(),
            span: span(),
        }),
        setup(Vec::new()),
    ]);
    assert!(analyze(&constant).is_err());

    let invalid_struct = module(vec![
        Item::Struct(StructDef {
            name: Symbol::new("Invalid"),
            fields: vec![polygl_hir::FieldDef {
                name: Symbol::new("value"),
                ty: Some(TypeExpr::new(TypeKind::Unit, span())),
                span: span(),
            }],
            methods: Vec::new(),
            span: span(),
        }),
        setup(Vec::new()),
    ]);
    assert!(analyze(&invalid_struct).is_err());

    let void_parameter = module(vec![Item::Entry(EntryPoint {
        kind: EntryPointKind::Vertex(Symbol::new("invalid")),
        params: vec![Param {
            name: Symbol::new("value"),
            ty: Some(TypeExpr::new(TypeKind::Unit, span())),
            span: span(),
        }],
        return_type: None,
        body: Block {
            statements: Vec::new(),
            span: span(),
        },
        span: span(),
    })]);
    assert!(analyze(&void_parameter).is_err());

    let identity = function(
        "identity",
        &["value"],
        vec![return_value(builder.variable("value"))],
    );
    let argument = module(vec![
        identity,
        setup(vec![
            builder.expression(call("identity", vec![void_call()])),
        ]),
    ]);
    assert!(analyze(&argument).is_err());

    let paint = function(
        "paint",
        &["x"],
        vec![return_value(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![
                builder.variable("x"),
                builder.float(0.0),
                builder.float(1.0),
            ],
        ))],
    );
    let typed = analyze(&module(vec![
        paint.clone(),
        setup(vec![
            builder.expression(call("paint", vec![builder.float(1.0)])),
        ]),
    ]))
    .expect("a final side-effect expression should infer a void helper");
    let helper = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("paint should be specialized");
    assert!(matches!(
        helper.return_type.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Unit)
    ));

    let invalid_result = module(vec![
        paint,
        setup(vec![builder.let_value(
            "invalid",
            call("paint", vec![builder.float(1.0)]),
        )]),
    ]);
    assert!(
        analyze(&invalid_result).is_err(),
        "a void helper remains invalid in a value binding"
    );
}

#[test]
fn rejects_malformed_or_unknown_hir_types() {
    let builder = HirBuilder::new(span());
    let unknown_struct = Item::Struct(StructDef {
        name: Symbol::new("Known"),
        fields: vec![polygl_hir::FieldDef {
            name: Symbol::new("missing"),
            ty: Some(TypeExpr::new(
                TypeKind::Struct(Symbol::new("Missing")),
                span(),
            )),
            span: span(),
        }],
        methods: Vec::new(),
        span: span(),
    });
    assert!(analyze(&module(vec![unknown_struct, setup(Vec::new())])).is_err());

    let mut malformed_matrix = function("matrix", &[], vec![return_value(builder.int(1))]);
    let Item::Function(malformed_function) = &mut malformed_matrix else {
        unreachable!("helper always returns a function");
    };
    malformed_function.return_type = Some(TypeExpr::new(TypeKind::Matrix(7), span()));
    assert!(analyze(&module(vec![malformed_matrix, setup(Vec::new())])).is_err());

    let malformed_vector = expression(ExprKind::Vector {
        size: 3,
        args: vec![builder.float(1.0)],
    });
    assert!(
        analyze(&module(vec![setup(vec![
            builder.expression(malformed_vector)
        ])]))
        .is_err()
    );

    let duplicate_fields = Item::Struct(StructDef {
        name: Symbol::new("Duplicate"),
        fields: vec![
            polygl_hir::FieldDef {
                name: Symbol::new("value"),
                ty: Some(TypeExpr::new(TypeKind::Int, span())),
                span: span(),
            },
            polygl_hir::FieldDef {
                name: Symbol::new("value"),
                ty: Some(TypeExpr::new(TypeKind::Int, span())),
                span: span(),
            },
        ],
        methods: Vec::new(),
        span: span(),
    });
    assert!(analyze(&module(vec![duplicate_fields, setup(Vec::new())])).is_err());

    let duplicate_parameters = function(
        "duplicate",
        &["value", "value"],
        vec![return_value(builder.int(1))],
    );
    assert!(analyze(&module(vec![duplicate_parameters, setup(Vec::new())])).is_err());

    assert!(
        analyze(&module(vec![setup(vec![Stmt::new(
            StmtKind::Break,
            span()
        )])]))
        .is_err()
    );

    let reserved_event = Item::Struct(StructDef {
        name: Symbol::new("Event"),
        fields: Vec::new(),
        methods: Vec::new(),
        span: span(),
    });
    assert!(analyze(&module(vec![reserved_event, setup(Vec::new())])).is_err());
}

#[test]
fn types_builtin_event_fields_from_the_registry() {
    let builder = HirBuilder::new(span());
    let field = |name| {
        expression(ExprKind::Field {
            base: Box::new(builder.variable("event")),
            field: Symbol::new(name),
        })
    };
    let typed = analyze(&module(vec![Item::Entry(EntryPoint {
        kind: EntryPointKind::OnEvent,
        params: vec![parameter("event")],
        return_type: None,
        body: Block {
            statements: vec![
                builder.expression(field("x")),
                builder.expression(field("key")),
            ],
            span: span(),
        },
        span: span(),
    })]))
    .expect("builtin Event fields should type-check");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected on_event");
    };
    let StmtKind::Expr(x) = &entry.body.statements[0].kind else {
        panic!("expected x field");
    };
    assert!(matches!(
        x.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    let StmtKind::Expr(key) = &entry.body.statements[1].kind else {
        panic!("expected key field");
    };
    assert!(matches!(
        key.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Option(inner)) if matches!(inner.kind, TypeKind::Str)
    ));
}

#[test]
fn rejects_a_ninth_function_instance_with_e0310() {
    let builder = HirBuilder::new(span());
    let identity = function(
        "identity",
        &["value"],
        vec![return_value(builder.variable("value"))],
    );
    let arguments = vec![
        builder.int(1),
        builder.float(1.0),
        builder.bool(true),
        builder.string("x"),
        expression(ExprKind::Array(vec![builder.int(1)])),
        expression(ExprKind::Array(vec![builder.float(1.0)])),
        expression(ExprKind::Array(vec![builder.bool(true)])),
        expression(ExprKind::Array(vec![builder.string("x")])),
        expression(ExprKind::Array(vec![expression(ExprKind::Array(vec![
            builder.int(1),
        ]))])),
    ];
    let calls = arguments
        .into_iter()
        .map(|argument| builder.expression(call("identity", vec![argument])))
        .collect();
    let diagnostics =
        analyze(&module(vec![identity, setup(calls)])).expect_err("ninth instance must fail");
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E0310")
        .expect("instance limit diagnostic");
    assert!(!diagnostic.suggestions.is_empty());
}

#[test]
fn requires_annotations_for_unresolved_aggregates() {
    let builder = HirBuilder::new(span());
    let hir = module(vec![setup(vec![
        builder.let_value("items", expression(ExprKind::Array(Vec::new()))),
    ])]);
    let diagnostics = analyze(&hir).expect_err("empty array has no element constraint");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0312")
    );

    let nil =
        module(vec![setup(vec![builder.let_value(
            "value",
            expression(ExprKind::Literal(Literal::None)),
        )])]);
    assert!(
        analyze(&nil).is_err(),
        "unconstrained nil must not produce Option<void>"
    );
}

#[test]
fn propagates_annotations_into_empty_aggregate_expressions() {
    let builder = HirBuilder::new(span());
    let binding = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("items"),
            ty: Some(TypeExpr::new(
                TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Float, span()))),
                span(),
            )),
            init: expression(ExprKind::Array(Vec::new())),
        },
        span(),
    );
    let typed = analyze(&module(vec![setup(vec![binding])]))
        .expect("the annotation should constrain the empty array");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { ty, init, .. } = &entry.body.statements[0].kind else {
        panic!("expected binding");
    };
    assert_eq!(ty, &init.ty);
    assert!(matches!(
        init.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Array(element)) if matches!(element.kind, TypeKind::Float)
    ));

    let empty = function(
        "empty",
        &[],
        vec![return_value(expression(ExprKind::Array(Vec::new())))],
    );
    let result = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("result"),
            ty: Some(TypeExpr::new(
                TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Str, span()))),
                span(),
            )),
            init: call("empty", Vec::new()),
        },
        span(),
    );
    let typed = analyze(&module(vec![empty, setup(vec![result])]))
        .expect("the caller result annotation should constrain the specialized body");
    let specialized = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) => Some(function),
            _ => None,
        })
        .expect("empty should be specialized");
    assert!(matches!(
        specialized.return_type.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Array(element)) if matches!(element.kind, TypeKind::Str)
    ));

    for annotated_first in [false, true] {
        let empty = function(
            "empty",
            &[],
            vec![return_value(expression(ExprKind::Array(Vec::new())))],
        );
        let unannotated = builder.let_value("unannotated", call("empty", Vec::new()));
        let annotated = Stmt::new(
            StmtKind::Let {
                name: Symbol::new("annotated"),
                ty: Some(TypeExpr::new(
                    TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Int, span()))),
                    span(),
                )),
                init: call("empty", Vec::new()),
            },
            span(),
        );
        let statements = if annotated_first {
            vec![annotated, unannotated]
        } else {
            vec![unannotated, annotated]
        };
        let typed = analyze(&module(vec![empty, setup(statements)]))
            .expect("same-key return constraints must be independent of call order");
        assert_eq!(typed.instance_count("empty"), 1);
    }

    for annotated_first in [false, true] {
        let empty = function(
            "empty",
            &[],
            vec![return_value(expression(ExprKind::Array(Vec::new())))],
        );
        let unannotated = builder.let_value("unannotated", call("empty", Vec::new()));
        let annotated = Stmt::new(
            StmtKind::Let {
                name: Symbol::new("annotated"),
                ty: Some(TypeExpr::new(
                    TypeKind::Array(Box::new(TypeExpr::new(TypeKind::Int, span()))),
                    span(),
                )),
                init: call("empty", Vec::new()),
            },
            span(),
        );
        let mut statements = if annotated_first {
            vec![annotated, unannotated]
        } else {
            vec![unannotated, annotated]
        };
        statements.push(return_value(builder.variable("dummy")));
        let wrapper = function("wrapper", &["dummy"], statements);
        let typed = analyze(&module(vec![
            empty,
            wrapper,
            setup(vec![
                builder.expression(call("wrapper", vec![builder.int(1)])),
            ]),
        ]))
        .expect("nested same-key calls must also be independent of call order");
        assert_eq!(typed.instance_count("empty"), 1);
    }

    let conflicting = function(
        "empty",
        &[],
        vec![return_value(expression(ExprKind::Array(Vec::new())))],
    );
    let annotated_call = |name, element| {
        Stmt::new(
            StmtKind::Let {
                name: Symbol::new(name),
                ty: Some(TypeExpr::new(
                    TypeKind::Array(Box::new(TypeExpr::new(element, span()))),
                    span(),
                )),
                init: call("empty", Vec::new()),
            },
            span(),
        )
    };
    assert!(
        analyze(&module(vec![
            conflicting,
            setup(vec![
                annotated_call("ints", TypeKind::Int),
                annotated_call("strings", TypeKind::Str),
            ]),
        ]))
        .is_err(),
        "one argument key cannot have conflicting return types"
    );
}

#[test]
fn preserves_numeric_constraints_and_strict_comparisons() {
    let builder = HirBuilder::new(span());
    let arithmetic = expression(ExprKind::Binary {
        op: BinOp::Add,
        left: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("left")),
            index: Box::new(builder.int(0)),
        })),
        right: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("right")),
            index: Box::new(builder.int(0)),
        })),
    });
    let assign_bool = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Index {
                    base: builder.variable("left"),
                    index: builder.int(0),
                },
                span: span(),
            },
            value: builder.bool(true),
        },
        span(),
    );
    let assign_other_bool = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Index {
                    base: builder.variable("right"),
                    index: builder.int(0),
                },
                span: span(),
            },
            value: builder.bool(false),
        },
        span(),
    );
    let diagnostics = analyze(&module(vec![setup(vec![
        builder.let_value("left", expression(ExprKind::Array(Vec::new()))),
        builder.let_value("right", expression(ExprKind::Array(Vec::new()))),
        builder.expression(arithmetic),
        assign_bool,
        assign_other_bool,
    ])]))
    .expect_err("addition must reject later bool bindings");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0303")
    );

    let string_add = expression(ExprKind::Binary {
        op: BinOp::Add,
        left: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("left")),
            index: Box::new(builder.int(0)),
        })),
        right: Box::new(expression(ExprKind::Index {
            base: Box::new(builder.variable("right")),
            index: Box::new(builder.int(0)),
        })),
    });
    let assign_string = |name: &str, value: &str| {
        Stmt::new(
            StmtKind::Assign {
                target: Place {
                    kind: PlaceKind::Index {
                        base: builder.variable(name),
                        index: builder.int(0),
                    },
                    span: span(),
                },
                value: builder.string(value),
            },
            span(),
        )
    };
    let typed = analyze(&module(vec![setup(vec![
        builder.let_value("left", expression(ExprKind::Array(Vec::new()))),
        builder.let_value("right", expression(ExprKind::Array(Vec::new()))),
        builder.let_value("joined", string_add),
        assign_string("left", "a"),
        assign_string("right", "b"),
    ])]))
    .expect("later string constraints should resolve ambiguous addition");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::Let { init, .. } = &entry.body.statements[2].kind else {
        panic!("expected joined binding");
    };
    assert!(matches!(
        init.kind,
        ExprKind::Binary {
            op: BinOp::StrConcat,
            ..
        }
    ));

    let comparison = expression(ExprKind::Binary {
        op: BinOp::Less,
        left: Box::new(builder.int(1)),
        right: Box::new(builder.float(2.0)),
    });
    let diagnostics = analyze(&module(vec![setup(vec![builder.expression(comparison)])]))
        .expect_err("ordered comparison operands must have identical types");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0303")
    );

    let constrained_later = module(vec![setup(vec![
        builder.let_value("value", builder.int(1)),
        builder.expression(expression(ExprKind::Binary {
            op: BinOp::Less,
            left: Box::new(builder.variable("value")),
            right: Box::new(builder.int(1)),
        })),
        builder.expression(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![
                builder.variable("value"),
                builder.float(0.0),
                builder.float(1.0),
            ],
        )),
    ])]);
    assert!(
        analyze(&constrained_later).is_err(),
        "a later widening must not invalidate an earlier exact comparison"
    );

    let negation = expression(ExprKind::Unary {
        op: UnOp::Neg,
        operand: Box::new(builder.bool(true)),
    });
    assert!(analyze(&module(vec![setup(vec![builder.expression(negation)])])).is_err());
}

#[test]
fn validates_return_annotations_fallthrough_and_entry_results() {
    let builder = HirBuilder::new(span());
    let mut annotated = function("answer", &[], vec![return_value(builder.string("wrong"))]);
    let Item::Function(annotated_function) = &mut annotated else {
        unreachable!("helper always returns a function");
    };
    annotated_function.return_type = Some(TypeExpr::new(TypeKind::Int, span()));
    let diagnostics = analyze(&module(vec![
        annotated,
        setup(vec![builder.expression(call("answer", Vec::new()))]),
    ]))
    .expect_err("the source return annotation must be enforced");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0303")
    );

    let partial = function(
        "partial",
        &["condition"],
        vec![Stmt::new(
            StmtKind::If {
                condition: builder.variable("condition"),
                then_block: Block {
                    statements: vec![return_value(builder.int(1))],
                    span: span(),
                },
                else_block: None,
            },
            span(),
        )],
    );
    assert!(
        analyze(&module(vec![
            partial,
            setup(vec![
                builder.expression(call("partial", vec![builder.bool(true)]))
            ]),
        ]))
        .is_err()
    );

    let entry = setup(vec![return_value(builder.int(1))]);
    assert!(analyze(&module(vec![entry])).is_err());

    let partial_shader = Item::Entry(EntryPoint {
        kind: EntryPointKind::Fragment(Symbol::new("partial")),
        params: Vec::new(),
        return_type: None,
        body: Block {
            statements: vec![Stmt::new(
                StmtKind::If {
                    condition: builder.bool(true),
                    then_block: Block {
                        statements: vec![return_value(expression(ExprKind::Vector {
                            size: 4,
                            args: vec![
                                builder.float(0.0),
                                builder.float(0.0),
                                builder.float(0.0),
                                builder.float(1.0),
                            ],
                        }))],
                        span: span(),
                    },
                    else_block: None,
                },
                span(),
            )],
            span: span(),
        },
        span: span(),
    });
    assert!(
        analyze(&module(vec![partial_shader])).is_err(),
        "every shader control-flow path must return the stage result"
    );
}

#[test]
fn constants_are_visible_and_keep_their_solved_types() {
    let builder = HirBuilder::new(span());
    let constant = Item::Const(ConstDef {
        name: Symbol::new("RED"),
        ty: None,
        value: builder.int(255),
        span: span(),
    });
    let typed = analyze(&module(vec![
        constant,
        setup(vec![builder.expression(builtin(
            polygl_hir::BuiltinId::BACKGROUND,
            vec![
                builder.variable("RED"),
                builder.float(0.0),
                builder.float(0.0),
            ],
        ))]),
    ]))
    .expect("constant references should participate in inference");
    let Item::Const(constant) = &typed.as_hir().items[0] else {
        panic!("expected retained constant");
    };
    assert!(matches!(
        constant.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));

    let derived = Item::Const(ConstDef {
        name: Symbol::new("DERIVED"),
        ty: None,
        value: expression(ExprKind::Binary {
            op: BinOp::DivInt,
            left: Box::new(builder.variable("BASE")),
            right: Box::new(builder.int(2)),
        }),
        span: span(),
    });
    let base = Item::Const(ConstDef {
        name: Symbol::new("BASE"),
        ty: None,
        value: builder.int(4),
        span: span(),
    });
    let typed = analyze(&module(vec![
        derived,
        base,
        setup(vec![builder.expression(builtin(
            polygl_hir::BuiltinId::CIRCLE,
            vec![
                builder.variable("BASE"),
                builder.float(0.0),
                builder.float(1.0),
            ],
        ))]),
    ]))
    .expect("derived constants should refresh after later constraints");
    let derived = typed
        .as_hir()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(constant) if constant.name.as_str() == "DERIVED" => Some(constant),
            _ => None,
        })
        .expect("derived constant");
    assert!(matches!(
        derived.ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
    assert!(matches!(
        derived.value.kind,
        ExprKind::Binary {
            op: BinOp::DivFloat,
            ..
        }
    ));

    let immutable = Item::Const(ConstDef {
        name: Symbol::new("BASE"),
        ty: None,
        value: expression(ExprKind::Array(vec![builder.int(1)])),
        span: span(),
    });
    let direct = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Var(Symbol::new("BASE")),
                span: span(),
            },
            value: expression(ExprKind::Array(vec![builder.int(2)])),
        },
        span(),
    );
    let indexed = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Index {
                    base: builder.variable("BASE"),
                    index: builder.int(0),
                },
                span: span(),
            },
            value: builder.int(2),
        },
        span(),
    );
    assert!(
        analyze(&module(vec![
            immutable.clone(),
            setup(vec![direct, indexed]),
        ]))
        .is_err(),
        "constants and their aggregate contents are immutable"
    );

    let local_shadow = Stmt::new(
        StmtKind::Assign {
            target: Place {
                kind: PlaceKind::Var(Symbol::new("BASE")),
                span: span(),
            },
            value: builder.int(2),
        },
        span(),
    );
    analyze(&module(vec![
        immutable,
        setup(vec![
            builder.let_value("BASE", builder.int(1)),
            local_shadow,
        ]),
    ]))
    .expect("a mutable local may shadow a constant");
}

#[test]
fn uses_node_identity_for_same_spanned_branch_bindings() {
    let builder = HirBuilder::new(span());
    let branch = Stmt::new(
        StmtKind::If {
            condition: builder.bool(true),
            then_block: Block {
                statements: vec![builder.let_value("value", builder.int(1))],
                span: span(),
            },
            else_block: Some(Block {
                statements: vec![builder.let_value("value", builder.float(1.0))],
                span: span(),
            }),
        },
        span(),
    );
    let typed =
        analyze(&module(vec![setup(vec![branch])])).expect("branch-local types are independent");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected setup");
    };
    let StmtKind::If {
        then_block,
        else_block: Some(else_block),
        ..
    } = &entry.body.statements[0].kind
    else {
        panic!("expected if");
    };
    let StmtKind::Let { ty: then_ty, .. } = &then_block.statements[0].kind else {
        panic!("expected then binding");
    };
    let StmtKind::Let { ty: else_ty, .. } = &else_block.statements[0].kind else {
        panic!("expected else binding");
    };
    assert!(matches!(
        then_ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Int)
    ));
    assert!(matches!(
        else_ty.as_ref().map(|ty| &ty.kind),
        Some(TypeKind::Float)
    ));
}

#[test]
fn generated_instance_names_are_injective() {
    let builder = HirBuilder::new(span());
    let definitions = ["A", "A_array"].into_iter().map(|name| {
        Item::Struct(StructDef {
            name: Symbol::new(name),
            fields: Vec::new(),
            methods: Vec::new(),
            span: span(),
        })
    });
    let construct = |name: &str| {
        expression(ExprKind::Struct {
            name: Symbol::new(name),
            fields: Vec::new(),
        })
    };
    let hir = module(
        definitions
            .chain([
                function(
                    "pair",
                    &["left", "right"],
                    vec![return_value(builder.variable("left"))],
                ),
                function(
                    "half",
                    &["value"],
                    vec![return_value(builder.variable("value"))],
                ),
                function(
                    "__pgl_4_half__int",
                    &["value"],
                    vec![return_value(builder.variable("value"))],
                ),
                setup(vec![
                    builder.expression(call(
                        "pair",
                        vec![
                            construct("A"),
                            expression(ExprKind::Array(vec![builder.int(1)])),
                        ],
                    )),
                    builder.expression(call("pair", vec![construct("A_array"), builder.int(1)])),
                    builder.expression(call("half", vec![builder.int(1)])),
                    builder.expression(call("__pgl_4_half__int", vec![builder.int(1)])),
                ]),
            ])
            .collect(),
    );
    let typed = analyze(&hir).expect("both structural tuples should specialize");
    let names = typed
        .as_hir()
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(function) => Some(function.name.as_str()),
            _ => None,
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(names.len(), 4);
}

#[test]
fn honors_source_annotations_and_reports_recursive_cycles() {
    let builder = HirBuilder::new(span());
    let annotated = Stmt::new(
        StmtKind::Let {
            name: Symbol::new("count"),
            ty: Some(TypeExpr::new(TypeKind::Int, span())),
            init: builder.float(1.0),
        },
        span(),
    );
    let diagnostics =
        analyze(&module(vec![setup(vec![annotated])])).expect_err("annotation must constrain");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0303")
    );

    let recursive = function(
        "again",
        &["value"],
        vec![return_value(call("again", vec![builder.variable("value")]))],
    );
    let diagnostics = analyze(&module(vec![
        recursive,
        setup(vec![
            builder.expression(call("again", vec![builder.int(1)])),
        ]),
    ]))
    .expect_err("recursive inference requires an explicit strategy");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0313")
    );
}

#[test]
fn expression_types_are_present_in_successful_typed_hir() {
    let builder = HirBuilder::new(span());
    let hir = module(vec![setup(vec![builder.expression(builtin(
        polygl_hir::BuiltinId::BACKGROUND,
        vec![
            expression(ExprKind::Literal(Literal::Float(0.0))),
            builder.float(0.0),
            builder.float(0.0),
        ],
    ))])]);
    let typed = analyze(&hir).expect("simple builtin call should type");
    let Item::Entry(entry) = &typed.as_hir().items[0] else {
        panic!("expected entry");
    };
    let StmtKind::Expr(expression) = &entry.body.statements[0].kind else {
        panic!("expected expression");
    };
    assert_eq!(
        expression.ty.as_ref().map(Type::from_expr),
        Some(Type::Unit)
    );
    let ExprKind::Call { args, .. } = &expression.kind else {
        panic!("expected call");
    };
    assert!(args.iter().all(|argument| argument.ty.is_some()));
}

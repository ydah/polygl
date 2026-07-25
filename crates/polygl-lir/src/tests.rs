use polygl_hir as hir;
use polygl_span::{SourceFile, SourceId, Span};

use crate::{BinaryOp, CallTarget, Domain, ExprKind, Literal, StatementKind, lower};

fn span() -> Span {
    SourceFile::new(SourceId::new(1), "test.rb", "x")
        .span(0, 1)
        .unwrap()
}

fn expression(kind: hir::ExprKind) -> hir::Expr {
    hir::Expr::new(kind, span())
}

fn call(name: &str, args: Vec<hir::Expr>) -> hir::Expr {
    expression(hir::ExprKind::Call {
        callee: hir::Callee::User(hir::Symbol::new(name)),
        args,
    })
}

fn builtin(id: hir::BuiltinId, args: Vec<hir::Expr>) -> hir::Expr {
    expression(hir::ExprKind::Call {
        callee: hir::Callee::Builtin(id),
        args,
    })
}

fn setup(statements: Vec<hir::Stmt>) -> hir::Item {
    hir::Item::Entry(hir::EntryPoint {
        kind: hir::EntryPointKind::Setup,
        params: Vec::new(),
        return_type: None,
        body: hir::Block {
            statements,
            span: span(),
        },
        span: span(),
    })
}

fn vertex(statements: Vec<hir::Stmt>) -> hir::Item {
    hir::Item::Entry(hir::EntryPoint {
        kind: hir::EntryPointKind::Vertex(hir::Symbol::new("main")),
        params: Vec::new(),
        return_type: None,
        body: hir::Block {
            statements,
            span: span(),
        },
        span: span(),
    })
}

fn module(items: Vec<hir::Item>) -> hir::Module {
    hir::Module {
        items,
        span: span(),
    }
}

#[test]
fn resolves_runtime_calls_defaults_and_void_returns() {
    let builder = hir::HirBuilder::new(span());
    let paint = hir::Item::Function(hir::Function {
        name: hir::Symbol::new("paint"),
        params: vec![hir::Param {
            name: hir::Symbol::new("x"),
            ty: None,
            span: span(),
        }],
        return_type: None,
        body: hir::Block {
            statements: vec![hir::Stmt::new(
                hir::StmtKind::Return(Some(builtin(
                    hir::BuiltinId::CIRCLE,
                    vec![
                        builder.variable("x"),
                        builder.float(0.0),
                        builder.float(1.0),
                    ],
                ))),
                span(),
            )],
            span: span(),
        },
        span: span(),
        domain: hir::DomainHint::Auto,
    });
    let sum = expression(hir::ExprKind::Binary {
        op: hir::BinOp::Add,
        left: Box::new(builder.int(1)),
        right: Box::new(builder.int(2)),
    });
    let typed = polygl_types::analyze(&module(vec![
        paint,
        setup(vec![
            builder.expression(builder.int(99)),
            builder.let_value("sum", sum),
            builder.expression(builtin(
                hir::BuiltinId::FILL,
                vec![builder.float(1.0), builder.float(0.5), builder.float(0.25)],
            )),
            builder.expression(call("paint", vec![builder.float(4.0)])),
        ]),
    ]))
    .expect("test HIR should type-check");

    let lir = lower(&typed);
    assert_eq!(lir.functions.len(), 1);
    assert_eq!(lir.functions[0].domain, Domain::Host);
    assert_eq!(lir.functions[0].body.statements.len(), 2);
    assert!(matches!(
        &lir.functions[0].body.statements[0].kind,
        StatementKind::Expr(crate::Expr {
            kind: ExprKind::Call {
                target: CallTarget::Runtime(operation),
                ..
            },
            ..
        }) if operation.as_str() == "circle"
    ));
    assert!(matches!(
        lir.functions[0].body.statements[1].kind,
        StatementKind::Return(None)
    ));

    let entry = &lir.entries[0];
    assert_eq!(entry.domain, Domain::Host);
    assert_eq!(
        entry.body.statements.len(),
        3,
        "literal expression statement is dead"
    );
    let StatementKind::Let { init, .. } = &entry.body.statements[0].kind else {
        panic!("expected folded binding");
    };
    assert!(matches!(init.kind, ExprKind::Literal(Literal::Int(3))));
    let StatementKind::Expr(fill) = &entry.body.statements[1].kind else {
        panic!("expected fill call");
    };
    let ExprKind::Call { target, args } = &fill.kind else {
        panic!("expected runtime call");
    };
    assert!(matches!(target, CallTarget::Runtime(operation) if operation.as_str() == "fill"));
    assert_eq!(args.len(), 4);
    assert!(matches!(
        args[3].kind,
        ExprKind::Literal(Literal::Float(1.0))
    ));
    let StatementKind::Expr(paint) = &entry.body.statements[2].kind else {
        panic!("expected user call");
    };
    assert!(matches!(
        &paint.kind,
        ExprKind::Call {
            target: CallTarget::Function(name),
            ..
        } if name.starts_with("__pgl_5_paint__")
    ));
    assert_eq!(paint.span, span());
}

#[test]
fn preserves_structured_control_flow_and_typed_data() {
    let builder = hir::HirBuilder::new(span());
    let point = hir::Item::Struct(hir::StructDef {
        name: hir::Symbol::new("Point"),
        fields: vec![hir::FieldDef {
            name: hir::Symbol::new("x"),
            ty: Some(hir::TypeExpr::new(hir::TypeKind::Float, span())),
            span: span(),
        }],
        methods: Vec::new(),
        span: span(),
    });
    let limit = hir::Item::Const(hir::ConstDef {
        name: hir::Symbol::new("LIMIT"),
        ty: None,
        value: builder.int(3),
        span: span(),
    });
    let assign = hir::Stmt::new(
        hir::StmtKind::Assign {
            target: hir::Place {
                kind: hir::PlaceKind::Index {
                    base: builder.variable("values"),
                    index: builder.int(0),
                },
                span: span(),
            },
            value: builder.int(2),
        },
        span(),
    );
    let point_value = expression(hir::ExprKind::Struct {
        name: hir::Symbol::new("Point"),
        fields: vec![hir::FieldInit {
            name: hir::Symbol::new("x"),
            value: builder.float(1.0),
            span: span(),
        }],
    });
    let field = expression(hir::ExprKind::Field {
        base: Box::new(builder.variable("point")),
        field: hir::Symbol::new("x"),
    });
    let branch = hir::Stmt::new(
        hir::StmtKind::If {
            condition: builder.bool(true),
            then_block: hir::Block {
                statements: vec![builder.expression(field)],
                span: span(),
            },
            else_block: None,
        },
        span(),
    );
    let loop_statement = hir::Stmt::new(
        hir::StmtKind::While {
            condition: builder.bool(true),
            body: hir::Block {
                statements: vec![hir::Stmt::new(hir::StmtKind::Break, span())],
                span: span(),
            },
        },
        span(),
    );
    let for_statement = hir::Stmt::new(
        hir::StmtKind::For {
            variable: hir::Symbol::new("i"),
            range: hir::RangeExpr {
                start: builder.int(0),
                end: builder.variable("LIMIT"),
                inclusive: true,
                span: span(),
            },
            body: hir::Block {
                statements: vec![hir::Stmt::new(hir::StmtKind::Continue, span())],
                span: span(),
            },
        },
        span(),
    );
    let typed = polygl_types::analyze(&module(vec![
        point,
        limit,
        setup(vec![
            builder.let_value("point", point_value),
            builder.let_value(
                "values",
                expression(hir::ExprKind::Array(vec![builder.int(1)])),
            ),
            assign,
            branch,
            loop_statement,
            for_statement,
        ]),
    ]))
    .expect("structured HIR should type-check");

    let lir = lower(&typed);
    assert_eq!(lir.structs[0].name, "Point");
    assert_eq!(lir.constants[0].name, "LIMIT");
    let statements = &lir.entries[0].body.statements;
    assert!(matches!(statements[2].kind, StatementKind::Assign { .. }));
    assert!(matches!(statements[3].kind, StatementKind::If { .. }));
    assert!(matches!(statements[4].kind, StatementKind::While { .. }));
    let StatementKind::For { range, body, .. } = &statements[5].kind else {
        panic!("expected structured for");
    };
    assert!(range.inclusive);
    assert!(matches!(body.statements[0].kind, StatementKind::Continue));
}

#[test]
fn leaves_trapping_integer_folds_for_runtime_evaluation() {
    let builder = hir::HirBuilder::new(span());
    let divide_by_zero = expression(hir::ExprKind::Binary {
        op: hir::BinOp::DivInt,
        left: Box::new(builder.int(1)),
        right: Box::new(builder.int(0)),
    });
    let typed = polygl_types::analyze(&module(vec![setup(vec![
        builder.let_value("value", divide_by_zero),
    ])]))
    .expect("division is well typed even when runtime-invalid");
    let lir = lower(&typed);
    let StatementKind::Let { init, .. } = &lir.entries[0].body.statements[0].kind else {
        panic!("expected binding");
    };
    assert!(matches!(
        init.kind,
        ExprKind::Binary {
            op: BinaryOp::IntegerDivide,
            ..
        }
    ));
}

#[test]
fn preserves_float_runtime_semantics_by_domain() {
    let builder = hir::HirBuilder::new(span());
    let gpu_math = hir::Item::Function(hir::Function {
        name: hir::Symbol::new("gpu_math"),
        params: Vec::new(),
        return_type: None,
        body: hir::Block {
            statements: vec![hir::Stmt::new(
                hir::StmtKind::Return(Some(expression(hir::ExprKind::Binary {
                    op: hir::BinOp::Sub,
                    left: Box::new(builder.float(16_777_217.0)),
                    right: Box::new(builder.float(16_777_216.0)),
                }))),
                span(),
            )],
            span: span(),
        },
        span: span(),
        domain: hir::DomainHint::Gpu,
    });
    let float_divide = expression(hir::ExprKind::Binary {
        op: hir::BinOp::DivFloat,
        left: Box::new(builder.float(1.0)),
        right: Box::new(builder.float(0.0)),
    });
    let typed = polygl_types::analyze(&module(vec![
        gpu_math,
        setup(vec![
            builder.let_value("gpu_value", call("gpu_math", Vec::new())),
            builder.let_value("host_value", float_divide),
        ]),
    ]))
    .expect("domain-sensitive arithmetic should type-check");

    let lir = lower(&typed);
    assert_eq!(lir.functions[0].domain, Domain::Gpu);
    let StatementKind::Return(Some(gpu_result)) = &lir.functions[0].body.statements[0].kind else {
        panic!("expected GPU return value");
    };
    assert!(matches!(
        gpu_result.kind,
        ExprKind::Binary {
            op: BinaryOp::Subtract,
            ..
        }
    ));
    let StatementKind::Let {
        init: host_result, ..
    } = &lir.entries[0].body.statements[1].kind
    else {
        panic!("expected host division binding");
    };
    assert!(matches!(
        host_result.kind,
        ExprKind::Binary {
            op: BinaryOp::FloatDivide,
            ..
        }
    ));
}

#[test]
fn propagates_constant_domains_and_preserves_local_shadowing() {
    let builder = hir::HirBuilder::new(span());
    let random_value = hir::Item::Const(hir::ConstDef {
        name: hir::Symbol::new("RANDOM_VALUE"),
        ty: None,
        value: builtin(
            hir::BuiltinId::RANDOM,
            vec![builder.float(0.0), builder.float(1.0)],
        ),
        span: span(),
    });
    let read_random = hir::Item::Function(hir::Function {
        name: hir::Symbol::new("read_random"),
        params: Vec::new(),
        return_type: None,
        body: hir::Block {
            statements: vec![hir::Stmt::new(
                hir::StmtKind::Return(Some(builder.variable("RANDOM_VALUE"))),
                span(),
            )],
            span: span(),
        },
        span: span(),
        domain: hir::DomainHint::Auto,
    });
    let shadowed = hir::Item::Const(hir::ConstDef {
        name: hir::Symbol::new("SHADOWED"),
        ty: None,
        value: builder.int(1),
        span: span(),
    });
    let typed = polygl_types::analyze(&module(vec![
        random_value,
        read_random,
        shadowed,
        vertex(vec![builder.expression(call("read_random", Vec::new()))]),
        setup(vec![
            builder.let_value("SHADOWED", builder.int(2)),
            builder.let_value("local_copy", builder.variable("SHADOWED")),
        ]),
    ]))
    .expect("constant dependencies and shadowing should type-check");

    let lir = lower(&typed);
    let random = lir
        .constants
        .iter()
        .find(|constant| constant.name == "RANDOM_VALUE")
        .expect("random constant");
    assert_eq!(random.domain, Domain::Host);
    assert_eq!(lir.functions[0].domain, Domain::Host);
    let StatementKind::Let { init, .. } = &lir.entries[1].body.statements[1].kind else {
        panic!("expected local copy binding");
    };
    assert!(matches!(init.kind, ExprKind::Variable(ref name) if name == "SHADOWED"));
}

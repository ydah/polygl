use polygl_span::{SourceFile, SourceId};

use crate::{BinOp, BuiltinId, EntryPointKind, ExprKind, HirBuilder, Item, StmtKind, dump};

#[test]
fn dumps_a_hand_written_triangle_program() {
    let source = SourceFile::new(SourceId::new(1), "triangle.rb", "triangle");
    let span = source.span(0, source.len()).unwrap();
    let builder = HirBuilder::new(span);
    let background = builder.builtin_call(
        BuiltinId::BACKGROUND,
        vec![builder.float(0.1), builder.float(0.1), builder.float(0.1)],
    );
    let fill = builder.builtin_call(
        BuiltinId::FILL,
        vec![builder.float(1.0), builder.float(0.2), builder.float(0.1)],
    );
    let triangle = builder.builtin_call(
        BuiltinId::TRIANGLE,
        vec![
            builder.float(10.0),
            builder.float(80.0),
            builder.float(50.0),
            builder.float(10.0),
            builder.float(90.0),
            builder.float(80.0),
        ],
    );
    let module = builder.module(vec![builder.entry(
        EntryPointKind::Setup,
        builder.block(vec![
            builder.expression(background),
            builder.expression(fill),
            builder.expression(triangle),
        ]),
    )]);

    let rendered = dump(&module);
    assert!(rendered.contains("entry setup() [host]"));
    assert!(rendered.contains("builtin#4(0.1, 0.1, 0.1);"));
    assert!(rendered.contains("builtin#11(10.0, 80.0, 50.0, 10.0, 90.0, 80.0);"));
    assert_eq!(module.span, span);
    let Item::Entry(entry) = &module.items[0] else {
        panic!("expected entry point");
    };
    assert!(entry.body.statements.iter().all(|stmt| stmt.span == span));
}

#[test]
fn dump_distinguishes_division_and_nil_check_nodes() {
    let source = SourceFile::new(SourceId::new(1), "ops.rb", "ops");
    let span = source.span(0, source.len()).unwrap();
    let builder = HirBuilder::new(span);
    let int_div = builder.binary(BinOp::DivInt, builder.int(5), builder.int(2));
    let float_div = builder.binary(BinOp::DivFloat, builder.float(5.0), builder.float(2.0));
    let nil_check = builder.nil_check(builder.variable("value"));
    let falsy_check = builder.falsy_check(builder.variable("condition"));
    let module = builder.module(vec![builder.entry(
        EntryPointKind::Setup,
        builder.block(vec![
            builder.let_value("integer", int_div),
            builder.let_value("floating", float_div),
            builder.expression(nil_check),
            builder.expression(falsy_check),
        ]),
    )]);

    let rendered = dump(&module);
    assert!(rendered.contains("(5 /int 2)"));
    assert!(rendered.contains("(5.0 /float 2.0)"));
    assert!(rendered.contains("nil?(value)"));
    assert!(rendered.contains("falsy?(condition)"));

    let Item::Entry(entry) = &module.items[0] else {
        panic!("expected entry");
    };
    let StmtKind::Let { init, .. } = &entry.body.statements[0].kind else {
        panic!("expected let");
    };
    assert!(matches!(
        init.kind,
        ExprKind::Binary {
            op: BinOp::DivInt,
            ..
        }
    ));
}

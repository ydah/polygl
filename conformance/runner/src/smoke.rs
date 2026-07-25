use std::path::Path;

use polygl_hir::{BuiltinId, EntryPointKind, HirBuilder, Module, dump};
use polygl_span::{SourceFile, SourceId};

use crate::snapshot::compare_l3_snapshot;
use crate::{
    ConformanceError, L1BaselineStore, L2SnapshotStore, NeutralProgram, RenderedFrame,
    compare_neutral_hir,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConformanceReport {
    pub l1_cases: usize,
    pub l2_cases: usize,
    pub l3_cases: usize,
}

pub fn verify_smoke(root: &Path) -> Result<ConformanceReport, ConformanceError> {
    let expected_frame = RenderedFrame {
        renderer: "swiftshader-smoke".to_owned(),
        width: 1,
        height: 1,
        rgba: vec![255, 64, 32, 255],
    };
    L1BaselineStore::new(root).verify("triangle", &expected_frame)?;

    let ruby = triangle_module();
    let php = triangle_module();
    let l2 = dump(&ruby);
    L2SnapshotStore::new(root).verify("handwritten", "triangle", &l2)?;

    let neutral = compare_neutral_hir(
        "triangle",
        &[
            NeutralProgram {
                language: "ruby",
                module: &ruby,
            },
            NeutralProgram {
                language: "php",
                module: &php,
            },
        ],
    )?;
    compare_l3_snapshot(root, "triangle", &neutral)?;

    Ok(ConformanceReport {
        l1_cases: 1,
        l2_cases: 1,
        l3_cases: 1,
    })
}

fn triangle_module() -> Module {
    let source = SourceFile::new(SourceId::new(0), "triangle", "triangle");
    let span = source.span(0, source.len()).expect("static span is valid");
    let builder = HirBuilder::new(span);
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
    builder.module(vec![builder.entry(
        EntryPointKind::Setup,
        builder.block(vec![builder.expression(triangle)]),
    )])
}

#[cfg(test)]
mod tests {
    use polygl_hir::dump;

    use crate::{NeutralProgram, compare_neutral_hir};

    use super::triangle_module;

    #[test]
    fn l2_triangle_dump_is_snapshot_stable() {
        insta::assert_snapshot!(dump(&triangle_module()), @r#"
        module
        {
          entry setup() [host]
          {
            builtin#11(10.0, 80.0, 50.0, 10.0, 90.0, 80.0);
          }
        }
        "#);
    }

    #[test]
    fn l3_rejects_duplicate_language_entries() {
        let module = triangle_module();
        assert!(
            compare_neutral_hir(
                "triangle",
                &[
                    NeutralProgram {
                        language: "ruby",
                        module: &module,
                    },
                    NeutralProgram {
                        language: "ruby",
                        module: &module,
                    },
                ],
            )
            .is_err()
        );
    }
}

use std::fs;
use std::path::Path;

use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_php::PhpAdapter;
use polygl_adapter_ruby::RubyAdapter;
use polygl_core::BuiltinTable;
use polygl_hir::{Module, dump};
use polygl_span::{Diagnostics, SourceFile, SourceId};

use crate::{
    ConformanceError, L1BaselineStore, L2SnapshotStore, L3SnapshotStore, NeutralProgram,
    compare_neutral_hir,
};

const M1_CASES: &[&str] = &[
    "background",
    "circle",
    "rectangle",
    "seeded-random",
    "triangle",
];
const NEUTRAL_CASES: &[&str] = &["rectangle", "triangle"];
const GPU_CASES: &[(&str, Option<&str>)] = &[
    ("plasma", None),
    ("gpu-string", Some("E0402")),
    ("gpu-host-call", Some("E0404")),
];
const BASELINE_RENDERER: &str = "swiftshader";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConformanceReport {
    pub l1_cases: usize,
    pub l2_cases: usize,
    pub l3_cases: usize,
    pub gpu_cases: usize,
}

pub fn verify_smoke(root: &Path) -> Result<ConformanceReport, ConformanceError> {
    let l1 = L1BaselineStore::new(root);
    let l2 = L2SnapshotStore::new(root);
    let l3 = L3SnapshotStore::new(root);

    for case in M1_CASES {
        let ruby = compile_ruby(root, case)?;
        let php = compile_php(root, case)?;
        l1.load(case, BASELINE_RENDERER)?;
        l2.verify("ruby", case, &dump(&ruby))?;
        l2.verify("php", case, &dump(&php))?;
        if NEUTRAL_CASES.contains(case) {
            compare_neutral_hir(
                case,
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
            l3.verify(case, &ruby)?;
        }
    }
    for (case, expected_error) in GPU_CASES {
        verify_gpu_case(root, case, *expected_error)?;
    }

    Ok(ConformanceReport {
        l1_cases: M1_CASES.len(),
        l2_cases: M1_CASES.len() * 2,
        l3_cases: NEUTRAL_CASES.len(),
        gpu_cases: GPU_CASES.len(),
    })
}

fn compile_ruby(root: &Path, case: &str) -> Result<Module, ConformanceError> {
    compile_typed(root, case, "main.rb", &RubyAdapter).map(polygl_types::TypedModule::into_hir)
}

fn compile_php(root: &Path, case: &str) -> Result<Module, ConformanceError> {
    compile_typed(root, case, "main.php", &PhpAdapter).map(polygl_types::TypedModule::into_hir)
}

fn compile_ruby_typed(
    root: &Path,
    case: &str,
) -> Result<polygl_types::TypedModule, ConformanceError> {
    compile_typed(root, case, "main.rb", &RubyAdapter)
}

fn compile_typed(
    root: &Path,
    case: &str,
    file: &str,
    adapter: &dyn LanguageAdapter,
) -> Result<polygl_types::TypedModule, ConformanceError> {
    let path = root.join("cases").join(case).join(file);
    let bytes = fs::read(&path)?;
    let source = SourceFile::from_bytes(SourceId::new(0), path.display().to_string(), bytes)
        .map_err(|error| ConformanceError::Compile {
            case: case.to_owned(),
            message: error.to_string(),
        })?;
    let mut context = LowerCtx::new(&BuiltinTable);
    let hir = adapter
        .lower(&source, &mut context)
        .map_err(|diagnostics| compile_diagnostics(case, &diagnostics, &source))?;
    polygl_types::analyze(&hir)
        .map_err(|diagnostics| compile_diagnostics(case, &diagnostics, &source))
}

fn verify_gpu_case(
    root: &Path,
    case: &str,
    expected_error: Option<&str>,
) -> Result<(), ConformanceError> {
    let typed = compile_ruby_typed(root, case)?;
    let lir = polygl_lir::lower(&typed);
    match (polygl_lir::split(&lir), expected_error) {
        (Ok(split), None) => {
            let artifacts = polygl_backend_glsl::GlslBackend::new()
                .generate(&split.gpu)
                .map_err(|error| ConformanceError::Compile {
                    case: case.to_owned(),
                    message: error.to_string(),
                })?;
            let shader = artifacts
                .shaders
                .first()
                .ok_or_else(|| ConformanceError::Compile {
                    case: case.to_owned(),
                    message: "positive GPU case did not emit a shader pair".to_owned(),
                })?;
            if !shader.vertex.starts_with("#version 300 es")
                || !shader.fragment.contains("uniform float u_time;")
            {
                return Err(ConformanceError::Compile {
                    case: case.to_owned(),
                    message: "positive GPU artifact is missing its GLSL version or u_time ABI"
                        .to_owned(),
                });
            }
            Ok(())
        }
        (Err(diagnostics), Some(expected))
            if diagnostics.iter().any(|item| item.code == expected) =>
        {
            Ok(())
        }
        (Err(diagnostics), expected) => Err(ConformanceError::Compile {
            case: case.to_owned(),
            message: format!(
                "expected GPU diagnostic {expected:?}, found {:?}",
                diagnostics
                    .iter()
                    .map(|item| item.code.as_str())
                    .collect::<Vec<_>>()
            ),
        }),
        (Ok(_), Some(expected)) => Err(ConformanceError::Compile {
            case: case.to_owned(),
            message: format!("expected GPU diagnostic {expected}, but split succeeded"),
        }),
    }
}

fn compile_diagnostics(
    case: &str,
    diagnostics: &Diagnostics,
    source: &SourceFile,
) -> ConformanceError {
    let message = diagnostics
        .render(source)
        .unwrap_or_else(|error| format!("diagnostic rendering failed: {error}"));
    ConformanceError::Compile {
        case: case.to_owned(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use polygl_adapter_api::{LanguageAdapter, LowerCtx};
    use polygl_adapter_php::PhpAdapter;
    use polygl_adapter_ruby::RubyAdapter;
    use polygl_core::BuiltinTable;
    use polygl_hir::dump;
    use polygl_span::{SourceFile, SourceId};

    use crate::{NeutralProgram, compare_neutral_hir};

    use super::{GPU_CASES, M1_CASES, NEUTRAL_CASES, compile_php, compile_ruby};

    #[test]
    fn m1_case_inventory_has_five_render_and_two_neutral_cases() {
        assert_eq!(M1_CASES.len(), 5);
        assert_eq!(NEUTRAL_CASES.len(), 2);
        assert!(NEUTRAL_CASES.iter().all(|case| M1_CASES.contains(case)));
        assert_eq!(GPU_CASES.len(), 3);
        let root = conformance_root();
        for case in M1_CASES {
            assert!(root.join("cases").join(case).join("main.php").is_file());
        }
    }

    #[test]
    fn l3_rejects_duplicate_language_entries() {
        let root = conformance_root();
        let module = compile_ruby(&root, "triangle").unwrap();
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

    #[test]
    fn neutral_cases_compare_ruby_and_php_before_snapshot_verification() {
        let root = conformance_root();
        for case in NEUTRAL_CASES {
            let ruby = compile_ruby(&root, case).unwrap();
            let php = compile_php(&root, case).unwrap();
            compare_neutral_hir(
                case,
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
            )
            .unwrap();
        }
    }

    #[test]
    fn division_difference_is_explicitly_language_defined() {
        let ruby = lower_text(
            "main.rb",
            "def half(value)\n  value / 2\nend\n",
            &RubyAdapter,
        );
        let php = lower_text(
            "main.php",
            "<?php function half($value) { return $value / 2; }",
            &PhpAdapter,
        );
        assert!(dump(&ruby).contains("(value /int 2)"));
        assert!(dump(&php).contains("(value /float 2)"));
    }

    fn lower_text(name: &str, text: &str, adapter: &dyn LanguageAdapter) -> polygl_hir::Module {
        let source = SourceFile::new(SourceId::new(9), name, text);
        adapter
            .lower(&source, &mut LowerCtx::new(&BuiltinTable))
            .unwrap()
    }

    fn conformance_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("runner lives below conformance")
            .to_path_buf()
    }
}

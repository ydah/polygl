use std::collections::HashSet;
use std::fs;
use std::path::Path;

use polygl_adapter_api::{LanguageAdapter, LowerCtx};
use polygl_adapter_perl::PerlAdapter;
use polygl_adapter_php::PhpAdapter;
use polygl_adapter_ruby::RubyAdapter;
use polygl_core::BuiltinTable;
use polygl_hir::dump;
use polygl_span::{Diagnostics, SourceFile, SourceId};

use crate::{
    ConformanceCase, ConformanceError, ConformanceLanguage, ConformanceLayer, L1BaselineStore,
    L2SnapshotStore, L3SnapshotStore, NeutralProgram, compare_neutral_hir, load_manifest,
    select_cases,
};

const BASELINE_RENDERER: &str = "swiftshader";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConformanceReport {
    pub l1_cases: usize,
    pub l2_cases: usize,
    pub l3_cases: usize,
    pub gpu_cases: usize,
}

pub fn verify_smoke(root: &Path) -> Result<ConformanceReport, ConformanceError> {
    let manifest = load_manifest(root)?;
    let selected = select_manifest_cases(&manifest)?;
    let l1 = L1BaselineStore::new(root);
    let l2 = L2SnapshotStore::new(root);
    let l3 = L3SnapshotStore::new(root);
    let mut report = ConformanceReport {
        l1_cases: 0,
        l2_cases: 0,
        l3_cases: 0,
        gpu_cases: 0,
    };

    for case in selected {
        if case.layers.contains(&ConformanceLayer::Gpu) {
            for language in &case.languages {
                let typed = compile_typed(root, case, *language)?;
                verify_gpu_program(
                    &case.id,
                    language.id(),
                    typed,
                    case.expected_diagnostic.as_deref(),
                )?;
            }
            report.gpu_cases += 1;
            continue;
        }

        let mut programs = Vec::with_capacity(case.languages.len());
        for language in &case.languages {
            let module = compile_typed(root, case, *language)?.into_hir();
            if case.layers.contains(&ConformanceLayer::L2HirSnapshot) {
                l2.verify(language.id(), &case.id, &dump(&module))?;
                report.l2_cases += 1;
            }
            programs.push((*language, module));
        }
        if case.layers.contains(&ConformanceLayer::L1Render) {
            l1.load(&case.id, BASELINE_RENDERER)?;
            report.l1_cases += 1;
        }
        if case.layers.contains(&ConformanceLayer::L3NeutralHir) {
            let neutral = programs
                .iter()
                .map(|(language, module)| NeutralProgram {
                    language: language.id(),
                    module,
                })
                .collect::<Vec<_>>();
            compare_neutral_hir(&case.id, &neutral)?;
            l3.verify(&case.id, &programs[0].1)?;
            report.l3_cases += 1;
        }
    }

    Ok(report)
}

fn select_manifest_cases(
    manifest: &[ConformanceCase],
) -> Result<Vec<&ConformanceCase>, ConformanceError> {
    let mut selected_ids = HashSet::new();
    for language in ConformanceLanguage::ALL {
        let capabilities = adapter(language).capabilities();
        for layer in [
            ConformanceLayer::L1Render,
            ConformanceLayer::L2HirSnapshot,
            ConformanceLayer::L3NeutralHir,
            ConformanceLayer::Gpu,
        ] {
            for case in select_cases(manifest, layer, capabilities) {
                if case.languages.contains(&language) {
                    selected_ids.insert(case.id.as_str());
                }
            }
        }
    }
    for case in manifest {
        validate_capabilities(case)?;
        if !selected_ids.contains(case.id.as_str()) {
            return Err(ConformanceError::InvalidManifest(format!(
                "case `{}` is not selected by any declared language capability set",
                case.id
            )));
        }
    }
    Ok(manifest
        .iter()
        .filter(|case| selected_ids.contains(case.id.as_str()))
        .collect())
}

fn validate_capabilities(case: &ConformanceCase) -> Result<(), ConformanceError> {
    for language in &case.languages {
        let capabilities = adapter(*language).capabilities();
        if let Some(feature) = case
            .required_features
            .iter()
            .find(|feature| !capabilities.contains(feature))
        {
            return Err(ConformanceError::InvalidManifest(format!(
                "case `{}` requires `{}` from {}, but its adapter does not advertise it",
                case.id,
                feature.as_str(),
                language.id()
            )));
        }
    }
    Ok(())
}

fn adapter(language: ConformanceLanguage) -> &'static dyn LanguageAdapter {
    match language {
        ConformanceLanguage::Ruby => &RubyAdapter,
        ConformanceLanguage::Php => &PhpAdapter,
        ConformanceLanguage::Perl => &PerlAdapter,
    }
}

fn compile_typed(
    root: &Path,
    case: &ConformanceCase,
    language: ConformanceLanguage,
) -> Result<polygl_types::TypedModule, ConformanceError> {
    let path = root.join("cases").join(&case.id).join(language.file());
    let bytes = fs::read(&path)?;
    let source = SourceFile::from_bytes(SourceId::new(0), path.display().to_string(), bytes)
        .map_err(|error| ConformanceError::Compile {
            case: case.id.clone(),
            message: error.to_string(),
        })?;
    let mut context = LowerCtx::new(&BuiltinTable);
    let hir = adapter(language)
        .lower(&source, &mut context)
        .map_err(|diagnostics| compile_diagnostics(&case.id, &diagnostics, &source))?;
    polygl_types::analyze(&hir)
        .map_err(|diagnostics| compile_diagnostics(&case.id, &diagnostics, &source))
}

fn verify_gpu_program(
    case: &str,
    language: &str,
    typed: polygl_types::TypedModule,
    expected_error: Option<&str>,
) -> Result<(), ConformanceError> {
    let lir = polygl_lir::lower(&typed);
    match (polygl_lir::split(&lir), expected_error) {
        (Ok(split), None) => {
            let artifacts = polygl_backend_glsl::GlslBackend::new()
                .generate(&split.gpu)
                .map_err(|error| ConformanceError::Compile {
                    case: format!("{case}/{language}"),
                    message: error.to_string(),
                })?;
            let shader = artifacts
                .shaders
                .first()
                .ok_or_else(|| ConformanceError::Compile {
                    case: format!("{case}/{language}"),
                    message: "positive GPU case did not emit a shader pair".to_owned(),
                })?;
            if !shader.vertex.starts_with("#version 300 es")
                || !shader.fragment.contains("uniform float u_time;")
            {
                return Err(ConformanceError::Compile {
                    case: format!("{case}/{language}"),
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
            case: format!("{case}/{language}"),
            message: format!(
                "expected GPU diagnostic {expected:?}, found {:?}",
                diagnostics
                    .iter()
                    .map(|item| item.code.as_str())
                    .collect::<Vec<_>>()
            ),
        }),
        (Ok(_), Some(expected)) => Err(ConformanceError::Compile {
            case: format!("{case}/{language}"),
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

    use polygl_adapter_api::{FeatureTag, LanguageAdapter, LowerCtx};
    use polygl_adapter_perl::PerlAdapter;
    use polygl_adapter_php::PhpAdapter;
    use polygl_adapter_ruby::RubyAdapter;
    use polygl_core::BuiltinTable;
    use polygl_hir::dump;
    use polygl_span::{SourceFile, SourceId};

    use crate::{ConformanceCase, ConformanceLanguage, ConformanceLayer, load_manifest};

    use super::select_manifest_cases;

    #[test]
    fn manifest_drives_every_smoke_case_and_feature() {
        let root = conformance_root();
        let cases = load_manifest(&root).unwrap();
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.layers.contains(&ConformanceLayer::L1Render))
                .count(),
            6
        );
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.layers.contains(&ConformanceLayer::L3NeutralHir))
                .count(),
            2
        );
        for case in cases {
            for language in case.languages {
                assert!(
                    root.join("cases")
                        .join(&case.id)
                        .join(language.file())
                        .is_file(),
                    "missing {}/{}",
                    case.id,
                    language.file()
                );
            }
        }
    }

    #[test]
    fn runner_rejects_a_language_without_the_required_capability() {
        let cases = [ConformanceCase {
            id: "php-truthiness".to_owned(),
            layers: vec![ConformanceLayer::L2HirSnapshot],
            languages: vec![ConformanceLanguage::Php],
            required_features: vec![FeatureTag::TruthinessSugar],
            expected_diagnostic: None,
            browser: false,
        }];
        let error = select_manifest_cases(&cases).unwrap_err().to_string();
        assert!(error.contains("truthiness-sugar-v1"));
        assert!(error.contains("php"));
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
        let perl = lower_text(
            "main.pl",
            "sub half { my ($value) = @_; return $value / 2; }\n",
            &PerlAdapter,
        );
        assert!(dump(&ruby).contains("(value /int 2)"));
        assert!(dump(&php).contains("(value /float 2)"));
        assert!(dump(&perl).contains("(value /float 2)"));
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

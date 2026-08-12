use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path};

use polygl_core::Compiler;
use polygl_span::{DiagnosticCode, SourceFile, SourceId};
use serde::Deserialize;

use crate::{ConformanceError, ConformanceLanguage};

#[derive(Deserialize)]
struct CorpusCase {
    id: String,
    language: ConformanceLanguage,
    source: String,
    outcome: CorpusOutcome,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
enum CorpusOutcome {
    Success,
    Diagnostics { code: String, minimum_count: usize },
}

/// Verifies parser recovery, annotation attachment, and identifier-policy
/// regressions declared in the shared adapter corpus.
pub fn verify_adapter_corpus(root: &Path) -> Result<usize, ConformanceError> {
    let path = root.join("adapter-corpus.json");
    let raw = fs::read_to_string(&path)?;
    let cases: Vec<CorpusCase> = serde_json::from_str(&raw).map_err(|error| {
        ConformanceError::InvalidManifest(format!("{}: {error}", path.display()))
    })?;
    let mut ids = HashSet::new();
    for case in &cases {
        if !ids.insert(case.id.as_str()) {
            return Err(ConformanceError::InvalidManifest(format!(
                "duplicate adapter corpus case `{}`",
                case.id
            )));
        }
        verify_case(root, case)?;
    }
    Ok(cases.len())
}

fn verify_case(root: &Path, case: &CorpusCase) -> Result<(), ConformanceError> {
    let relative = Path::new(&case.source);
    if relative.is_absolute()
        || !relative.starts_with("adapter-corpus")
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ConformanceError::InvalidManifest(format!(
            "adapter corpus case `{}` has non-portable source path `{}`",
            case.id, case.source
        )));
    }
    let path = root.join(relative);
    let bytes = fs::read(&path)?;
    let source =
        SourceFile::from_bytes(SourceId::new(0), case.source.clone(), bytes).map_err(|error| {
            ConformanceError::Compile {
                case: case.id.clone(),
                message: error.to_string(),
            }
        })?;
    let result = Compiler::standard().analyze(&source, case.language.id());
    match (&case.outcome, result) {
        (CorpusOutcome::Success, Ok(_)) => Ok(()),
        (CorpusOutcome::Success, Err(error)) => Err(ConformanceError::Compile {
            case: case.id.clone(),
            message: error.render(&source),
        }),
        (
            CorpusOutcome::Diagnostics {
                code,
                minimum_count,
            },
            Err(error),
        ) => {
            let expected = DiagnosticCode::parse(code).ok_or_else(|| {
                ConformanceError::InvalidManifest(format!(
                    "adapter corpus case `{}` names unregistered diagnostic `{code}`",
                    case.id
                ))
            })?;
            if *minimum_count == 0 {
                return Err(ConformanceError::InvalidManifest(format!(
                    "adapter corpus case `{}` has a zero diagnostic minimum",
                    case.id
                )));
            }
            let diagnostics = error
                .diagnostics()
                .ok_or_else(|| ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!("expected structured diagnostic {expected}, got {error}"),
                })?;
            diagnostics
                .validate()
                .map_err(|reason| ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!("invalid diagnostic contract: {reason}"),
                })?;
            for diagnostic in diagnostics.iter() {
                diagnostic
                    .primary_span
                    .validate_for(&source)
                    .map_err(|error| ConformanceError::Compile {
                        case: case.id.clone(),
                        message: format!("invalid primary diagnostic span: {error}"),
                    })?;
                for span in diagnostic.labels.iter().map(|label| label.span).chain(
                    diagnostic
                        .suggestions
                        .iter()
                        .map(|suggestion| suggestion.span),
                ) {
                    span.validate_for(&source)
                        .map_err(|error| ConformanceError::Compile {
                            case: case.id.clone(),
                            message: format!("invalid related diagnostic span: {error}"),
                        })?;
                }
            }
            let count = diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == expected)
                .count();
            if count < *minimum_count {
                return Err(ConformanceError::Compile {
                    case: case.id.clone(),
                    message: format!(
                        "expected at least {minimum_count} {expected} diagnostics, got {count}: {}",
                        error.render(&source)
                    ),
                });
            }
            Ok(())
        }
        (CorpusOutcome::Diagnostics { code, .. }, Ok(_)) => Err(ConformanceError::Compile {
            case: case.id.clone(),
            message: format!("expected diagnostic {code}, but compilation succeeded"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::verify_adapter_corpus;

    #[test]
    fn shared_adapter_corpus_passes() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .expect("runner lives directly under conformance");
        assert_eq!(verify_adapter_corpus(root).unwrap(), 12);
    }
}

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use polygl_hir::{Module, normalized_dump};

use crate::ConformanceError;

pub struct L2SnapshotStore {
    root: PathBuf,
}

impl L2SnapshotStore {
    #[must_use]
    pub fn new(conformance_root: impl Into<PathBuf>) -> Self {
        Self {
            root: conformance_root.into(),
        }
    }

    pub fn verify(&self, language: &str, case: &str, actual: &str) -> Result<(), ConformanceError> {
        validate_name(language)?;
        validate_name(case)?;
        let path = self
            .root
            .join("l2-hir-snapshots")
            .join(case)
            .join(format!("{language}.hir"));
        compare_snapshot(&path, "L2", case, language, actual)
    }
}

pub struct NeutralProgram<'a> {
    pub language: &'a str,
    pub module: &'a Module,
}

pub fn compare_neutral_hir(
    case: &str,
    programs: &[NeutralProgram<'_>],
) -> Result<String, ConformanceError> {
    validate_name(case)?;
    if programs.len() < 2 {
        return Err(ConformanceError::NotEnoughNeutralPrograms);
    }
    let mut languages = HashSet::new();
    for program in programs {
        validate_name(program.language)?;
        if !languages.insert(program.language) {
            return Err(ConformanceError::DuplicateLanguage(
                program.language.to_owned(),
            ));
        }
    }
    let expected = normalized_dump(programs[0].module);
    for program in &programs[1..] {
        if normalized_dump(program.module) != expected {
            return Err(ConformanceError::SnapshotMismatch {
                layer: "L3",
                case: case.to_owned(),
                subject: program.language.to_owned(),
            });
        }
    }
    Ok(expected)
}

pub(crate) fn compare_l3_snapshot(
    root: &Path,
    case: &str,
    actual: &str,
) -> Result<(), ConformanceError> {
    let path = root.join("l3-neutral-hir").join(case).join("neutral.hir");
    compare_snapshot(&path, "L3", case, "neutral", actual)
}

fn compare_snapshot(
    path: &Path,
    layer: &'static str,
    case: &str,
    subject: &str,
    actual: &str,
) -> Result<(), ConformanceError> {
    let expected = fs::read_to_string(path)?;
    if expected != actual {
        return Err(ConformanceError::SnapshotMismatch {
            layer,
            case: case.to_owned(),
            subject: subject.to_owned(),
        });
    }
    Ok(())
}

fn validate_name(value: &str) -> Result<(), ConformanceError> {
    let valid = !value.is_empty()
        && value
            .chars()
            .all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || "-_".contains(char));
    if valid {
        Ok(())
    } else {
        Err(ConformanceError::InvalidName(value.to_owned()))
    }
}

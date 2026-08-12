use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use polygl_adapter_api::FeatureTag;
use polygl_span::DiagnosticCode;
use serde::Deserialize;

use crate::ConformanceError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConformanceLayer {
    L1Render,
    L2HirSnapshot,
    L3NeutralHir,
    Gpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConformanceLanguage {
    Ruby,
    Php,
    Perl,
}

impl ConformanceLanguage {
    pub const ALL: [Self; 3] = [Self::Ruby, Self::Php, Self::Perl];

    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Perl => "perl",
        }
    }

    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Ruby => "main.rb",
            Self::Php => "main.php",
            Self::Perl => "main.pl",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceCase {
    pub id: String,
    pub layers: Vec<ConformanceLayer>,
    pub languages: Vec<ConformanceLanguage>,
    pub required_features: Vec<FeatureTag>,
    pub expected_diagnostic: Option<String>,
    pub browser: bool,
}

#[derive(Deserialize)]
struct ManifestCase {
    id: String,
    layers: Vec<ConformanceLayer>,
    languages: Vec<ConformanceLanguage>,
    required_features: Vec<String>,
    #[serde(default)]
    expected_diagnostic: Option<String>,
    browser: bool,
}

pub fn load_manifest(root: &Path) -> Result<Vec<ConformanceCase>, ConformanceError> {
    let path = root.join("cases.json");
    let source = fs::read_to_string(&path)?;
    let raw: Vec<ManifestCase> = serde_json::from_str(&source).map_err(|error| {
        ConformanceError::InvalidManifest(format!("{}: {error}", path.display()))
    })?;
    let mut ids = HashSet::new();
    let mut feature_coverage = HashMap::<FeatureTag, usize>::new();
    let mut cases = Vec::with_capacity(raw.len());
    for item in raw {
        validate_name(&item.id)?;
        if !ids.insert(item.id.clone()) {
            return Err(ConformanceError::InvalidManifest(format!(
                "duplicate case id `{}`",
                item.id
            )));
        }
        if item.layers.is_empty() || item.languages.is_empty() {
            return Err(ConformanceError::InvalidManifest(format!(
                "case `{}` must declare at least one layer and language",
                item.id
            )));
        }
        if item.expected_diagnostic.is_some() && !item.layers.contains(&ConformanceLayer::Gpu) {
            return Err(ConformanceError::InvalidManifest(format!(
                "case `{}` has an expected diagnostic outside the GPU layer",
                item.id
            )));
        }
        if let Some(code) = &item.expected_diagnostic
            && DiagnosticCode::parse(code).is_none()
        {
            return Err(ConformanceError::InvalidManifest(format!(
                "case `{}` names unregistered diagnostic `{code}`",
                item.id
            )));
        }
        let required_features = item
            .required_features
            .iter()
            .map(|name| {
                FeatureTag::parse(name).ok_or_else(|| {
                    ConformanceError::InvalidManifest(format!(
                        "case `{}` names unknown feature `{name}`",
                        item.id
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        for feature in &required_features {
            *feature_coverage.entry(*feature).or_default() += 1;
        }
        cases.push(ConformanceCase {
            id: item.id,
            layers: item.layers,
            languages: item.languages,
            required_features,
            expected_diagnostic: item.expected_diagnostic,
            browser: item.browser,
        });
    }
    for feature in FeatureTag::ALL {
        if feature_coverage.get(&feature).copied().unwrap_or_default() == 0 {
            return Err(ConformanceError::InvalidManifest(format!(
                "feature `{}` has no conformance case",
                feature.as_str()
            )));
        }
    }
    Ok(cases)
}

#[must_use]
pub fn select_cases<'a>(
    cases: &'a [ConformanceCase],
    layer: ConformanceLayer,
    capabilities: &[FeatureTag],
) -> Vec<&'a ConformanceCase> {
    let capabilities = capabilities.iter().copied().collect::<HashSet<_>>();
    cases
        .iter()
        .filter(|case| case.layers.contains(&layer))
        .filter(|case| {
            case.required_features
                .iter()
                .all(|feature| capabilities.contains(feature))
        })
        .collect()
}

fn validate_name(value: &str) -> Result<(), ConformanceError> {
    let valid = !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        });
    if valid {
        Ok(())
    } else {
        Err(ConformanceError::InvalidName(value.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use polygl_adapter_api::FeatureTag;

    use super::{
        ConformanceCase, ConformanceLanguage, ConformanceLayer, load_manifest, select_cases,
    };

    #[test]
    fn selects_only_layer_and_capability_compatible_cases() {
        let cases = [
            ConformanceCase {
                id: "triangle".to_owned(),
                layers: vec![ConformanceLayer::L1Render],
                languages: ConformanceLanguage::ALL.to_vec(),
                required_features: vec![FeatureTag::Core, FeatureTag::Tier1],
                expected_diagnostic: None,
                browser: true,
            },
            ConformanceCase {
                id: "truthiness".to_owned(),
                layers: vec![ConformanceLayer::L2HirSnapshot],
                languages: vec![ConformanceLanguage::Ruby],
                required_features: vec![FeatureTag::TruthinessSugar],
                expected_diagnostic: None,
                browser: false,
            },
        ];
        let selected = select_cases(
            &cases,
            ConformanceLayer::L1Render,
            &[FeatureTag::Tier1, FeatureTag::Core],
        );
        assert_eq!(
            selected
                .iter()
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>(),
            ["triangle"]
        );
    }

    #[test]
    fn manifest_requires_coverage_for_every_feature_tag() {
        let temporary = tempfile::tempdir().unwrap();
        fs::write(
            temporary.path().join("cases.json"),
            r#"[{
              "id": "core-only",
              "layers": ["l2-hir-snapshot"],
              "languages": ["ruby"],
              "required_features": ["core-v1"],
              "browser": false
            }]"#,
        )
        .unwrap();
        let error = load_manifest(temporary.path()).unwrap_err().to_string();
        assert!(error.contains("has no conformance case"));
    }
}

use std::collections::HashSet;

use polygl_adapter_api::FeatureTag;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConformanceLayer {
    L1Render,
    L2HirSnapshot,
    L3NeutralHir,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConformanceCase {
    pub name: &'static str,
    pub layer: ConformanceLayer,
    pub required_features: &'static [FeatureTag],
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
        .filter(|case| case.layer == layer)
        .filter(|case| {
            case.required_features
                .iter()
                .all(|feature| capabilities.contains(feature))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use polygl_adapter_api::FeatureTag;

    use super::{ConformanceCase, ConformanceLayer, select_cases};

    const CASES: &[ConformanceCase] = &[
        ConformanceCase {
            name: "triangle",
            layer: ConformanceLayer::L1Render,
            required_features: &[FeatureTag::Core, FeatureTag::Tier1],
        },
        ConformanceCase {
            name: "truthiness",
            layer: ConformanceLayer::L1Render,
            required_features: &[FeatureTag::TruthinessSugar],
        },
    ];

    #[test]
    fn selects_only_layer_and_capability_compatible_cases() {
        let selected = select_cases(
            CASES,
            ConformanceLayer::L1Render,
            &[FeatureTag::Tier1, FeatureTag::Core],
        );
        assert_eq!(
            selected.iter().map(|case| case.name).collect::<Vec<_>>(),
            ["triangle"]
        );
    }
}

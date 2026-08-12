#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FeatureTag {
    Core,
    Tier1,
    Tier2,
    Arrays,
    Maps,
    Classes,
    TimesBlockSugar,
    EachBlockSugar,
    TruthinessSugar,
    Shaders,
}

impl FeatureTag {
    pub const ALL: [Self; 10] = [
        Self::Core,
        Self::Tier1,
        Self::Tier2,
        Self::Arrays,
        Self::Maps,
        Self::Classes,
        Self::TimesBlockSugar,
        Self::EachBlockSugar,
        Self::TruthinessSugar,
        Self::Shaders,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core-v1",
            Self::Tier1 => "tier1-v1",
            Self::Tier2 => "tier2-v1",
            Self::Arrays => "arrays-v1",
            Self::Maps => "maps-v1",
            Self::Classes => "classes-v1",
            Self::TimesBlockSugar => "times-block-sugar-v1",
            Self::EachBlockSugar => "each-block-sugar-v1",
            Self::TruthinessSugar => "truthiness-sugar-v1",
            Self::Shaders => "shaders-v1",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.as_str() == value)
    }
}

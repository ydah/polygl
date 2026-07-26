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
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Tier1 => "tier1",
            Self::Tier2 => "tier2",
            Self::Arrays => "arrays",
            Self::Maps => "maps",
            Self::Classes => "classes",
            Self::TimesBlockSugar => "times-block-sugar",
            Self::EachBlockSugar => "each-block-sugar",
            Self::TruthinessSugar => "truthiness-sugar",
            Self::Shaders => "shaders",
        }
    }
}

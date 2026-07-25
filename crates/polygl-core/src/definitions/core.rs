use super::{Builtin, BuiltinId, BuiltinTier, Domain, F, I, builtin, req};

pub(super) const FLOOR: Builtin = builtin(
    BuiltinId::FLOOR,
    "floor",
    BuiltinTier::Core,
    &[req("value", F)],
    I,
    Domain::Both,
    "floorToInt",
);

pub(super) const ROUND: Builtin = builtin(
    BuiltinId::ROUND,
    "round",
    BuiltinTier::Core,
    &[req("value", F)],
    I,
    Domain::Both,
    "roundToInt",
);

pub(super) const TRUNC: Builtin = builtin(
    BuiltinId::TRUNC,
    "trunc",
    BuiltinTier::Core,
    &[req("value", F)],
    I,
    Domain::Both,
    "truncToInt",
);

use super::{Builtin, BuiltinId, BuiltinTier, Domain, F, I, S, V, alpha, builtin, req};

pub(super) const SIZE: Builtin = builtin(
    BuiltinId::SIZE,
    "size",
    BuiltinTier::Tier1,
    &[req("w", I), req("h", I)],
    V,
    Domain::Host,
    "size",
);
pub(super) const BACKGROUND: Builtin = builtin(
    BuiltinId::BACKGROUND,
    "background",
    BuiltinTier::Tier1,
    &[req("r", F), req("g", F), req("b", F)],
    V,
    Domain::Host,
    "background",
);
pub(super) const FILL: Builtin = builtin(
    BuiltinId::FILL,
    "fill",
    BuiltinTier::Tier1,
    &[req("r", F), req("g", F), req("b", F), alpha()],
    V,
    Domain::Host,
    "fill",
);
pub(super) const STROKE: Builtin = builtin(
    BuiltinId::STROKE,
    "stroke",
    BuiltinTier::Tier1,
    &[req("r", F), req("g", F), req("b", F), alpha()],
    V,
    Domain::Host,
    "stroke",
);
pub(super) const NO_STROKE: Builtin = builtin(
    BuiltinId::NO_STROKE,
    "no_stroke",
    BuiltinTier::Tier1,
    &[],
    V,
    Domain::Host,
    "noStroke",
);
pub(super) const RECT: Builtin = builtin(
    BuiltinId::RECT,
    "rect",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F), req("w", F), req("h", F)],
    V,
    Domain::Host,
    "rect",
);
pub(super) const CIRCLE: Builtin = builtin(
    BuiltinId::CIRCLE,
    "circle",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F), req("r", F)],
    V,
    Domain::Host,
    "circle",
);
pub(super) const LINE: Builtin = builtin(
    BuiltinId::LINE,
    "line",
    BuiltinTier::Tier1,
    &[req("x1", F), req("y1", F), req("x2", F), req("y2", F)],
    V,
    Domain::Host,
    "line",
);
pub(super) const TRIANGLE: Builtin = builtin(
    BuiltinId::TRIANGLE,
    "triangle",
    BuiltinTier::Tier1,
    &[
        req("x1", F),
        req("y1", F),
        req("x2", F),
        req("y2", F),
        req("x3", F),
        req("y3", F),
    ],
    V,
    Domain::Host,
    "triangle",
);
pub(super) const TEXT: Builtin = builtin(
    BuiltinId::TEXT,
    "text",
    BuiltinTier::Tier1,
    &[req("s", S), req("x", F), req("y", F)],
    V,
    Domain::Host,
    "text",
);

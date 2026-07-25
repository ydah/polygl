use super::{B, Builtin, BuiltinId, BuiltinTier, Domain, F, I, S, V, builtin, req};

pub(super) const PUSH_MATRIX: Builtin = builtin(
    BuiltinId::PUSH_MATRIX,
    "push_matrix",
    BuiltinTier::Tier1,
    &[],
    V,
    Domain::Host,
    "pushMatrix",
);
pub(super) const POP_MATRIX: Builtin = builtin(
    BuiltinId::POP_MATRIX,
    "pop_matrix",
    BuiltinTier::Tier1,
    &[],
    V,
    Domain::Host,
    "popMatrix",
);
pub(super) const TRANSLATE: Builtin = builtin(
    BuiltinId::TRANSLATE,
    "translate",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F)],
    V,
    Domain::Host,
    "translate",
);
pub(super) const ROTATE: Builtin = builtin(
    BuiltinId::ROTATE,
    "rotate",
    BuiltinTier::Tier1,
    &[req("radians", F)],
    V,
    Domain::Host,
    "rotate",
);
pub(super) const SCALE: Builtin = builtin(
    BuiltinId::SCALE,
    "scale",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F)],
    V,
    Domain::Host,
    "scale",
);
pub(super) const WIDTH: Builtin = builtin(
    BuiltinId::WIDTH,
    "width",
    BuiltinTier::Tier1,
    &[],
    I,
    Domain::Host,
    "width",
);
pub(super) const HEIGHT: Builtin = builtin(
    BuiltinId::HEIGHT,
    "height",
    BuiltinTier::Tier1,
    &[],
    I,
    Domain::Host,
    "height",
);
pub(super) const TIME: Builtin = builtin(
    BuiltinId::TIME,
    "time",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Both,
    "time",
);
pub(super) const MOUSE_X: Builtin = builtin(
    BuiltinId::MOUSE_X,
    "mouse_x",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Host,
    "mouseX",
);
pub(super) const MOUSE_Y: Builtin = builtin(
    BuiltinId::MOUSE_Y,
    "mouse_y",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Host,
    "mouseY",
);
pub(super) const KEY_DOWN: Builtin = builtin(
    BuiltinId::KEY_DOWN,
    "key_down",
    BuiltinTier::Tier1,
    &[req("key", S)],
    B,
    Domain::Host,
    "keyDown",
);
pub(super) const RANDOM: Builtin = builtin(
    BuiltinId::RANDOM,
    "random",
    BuiltinTier::Tier1,
    &[req("a", F), req("b", F)],
    F,
    Domain::Host,
    "random",
);

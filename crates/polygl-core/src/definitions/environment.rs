use super::{B, Builtin, BuiltinId, BuiltinTier, Domain, F, I, S, V, builtin, req};

pub(super) const PUSH_MATRIX: Builtin = builtin(
    BuiltinId::PushMatrix,
    "push_matrix",
    BuiltinTier::Tier1,
    &[],
    V,
    Domain::Host,
    "pushMatrix",
);
pub(super) const POP_MATRIX: Builtin = builtin(
    BuiltinId::PopMatrix,
    "pop_matrix",
    BuiltinTier::Tier1,
    &[],
    V,
    Domain::Host,
    "popMatrix",
);
pub(super) const TRANSLATE: Builtin = builtin(
    BuiltinId::Translate,
    "translate",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F)],
    V,
    Domain::Host,
    "translate",
);
pub(super) const ROTATE: Builtin = builtin(
    BuiltinId::Rotate,
    "rotate",
    BuiltinTier::Tier1,
    &[req("radians", F)],
    V,
    Domain::Host,
    "rotate",
);
pub(super) const SCALE: Builtin = builtin(
    BuiltinId::Scale,
    "scale",
    BuiltinTier::Tier1,
    &[req("x", F), req("y", F)],
    V,
    Domain::Host,
    "scale",
);
pub(super) const WIDTH: Builtin = builtin(
    BuiltinId::Width,
    "width",
    BuiltinTier::Tier1,
    &[],
    I,
    Domain::Host,
    "width",
);
pub(super) const HEIGHT: Builtin = builtin(
    BuiltinId::Height,
    "height",
    BuiltinTier::Tier1,
    &[],
    I,
    Domain::Host,
    "height",
);
pub(super) const TIME: Builtin = builtin(
    BuiltinId::Time,
    "time",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Both,
    "time",
);
pub(super) const MOUSE_X: Builtin = builtin(
    BuiltinId::MouseX,
    "mouse_x",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Host,
    "mouseX",
);
pub(super) const MOUSE_Y: Builtin = builtin(
    BuiltinId::MouseY,
    "mouse_y",
    BuiltinTier::Tier1,
    &[],
    F,
    Domain::Host,
    "mouseY",
);
pub(super) const KEY_DOWN: Builtin = builtin(
    BuiltinId::KeyDown,
    "key_down",
    BuiltinTier::Tier1,
    &[req("key", S)],
    B,
    Domain::Host,
    "keyDown",
);
pub(super) const RANDOM: Builtin = builtin(
    BuiltinId::Random,
    "random",
    BuiltinTier::Tier1,
    &[req("a", F), req("b", F)],
    F,
    Domain::Host,
    "random",
);

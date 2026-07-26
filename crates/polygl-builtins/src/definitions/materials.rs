use polygl_hir::OpaqueType;

use super::{Builtin, BuiltinId, BuiltinTier, Domain, S, builtin, req};
use crate::BuiltinType;

pub(super) const MATERIAL_SHADER: Builtin = builtin(
    BuiltinId::MATERIAL_SHADER,
    "material_shader",
    BuiltinTier::Tier2,
    &[req("name", S)],
    BuiltinType::Opaque(OpaqueType::Material),
    Domain::Host,
    "materialShader",
);

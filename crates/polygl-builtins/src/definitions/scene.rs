use polygl_hir::OpaqueType;

use super::{
    Builtin, BuiltinId, BuiltinTier, BuiltinType, DefaultValue, Domain, F, I, S, V, builtin, req,
};

const MESH: BuiltinType = BuiltinType::Opaque(OpaqueType::Mesh);
const NODE: BuiltinType = BuiltinType::Opaque(OpaqueType::Node);
const MATERIAL: BuiltinType = BuiltinType::Opaque(OpaqueType::Material);
const TEXTURE: BuiltinType = BuiltinType::Opaque(OpaqueType::Texture);

pub(super) const MESH_BOX: Builtin = builtin(
    BuiltinId::MESH_BOX,
    "mesh_box",
    BuiltinTier::Tier2,
    &[req("w", F), req("h", F), req("d", F)],
    MESH,
    Domain::Host,
    "meshBox",
);

pub(super) const MESH_SPHERE: Builtin = builtin(
    BuiltinId::MESH_SPHERE,
    "mesh_sphere",
    BuiltinTier::Tier2,
    &[req("r", F), req("segments", I)],
    MESH,
    Domain::Host,
    "meshSphere",
);

pub(super) const MESH_PLANE: Builtin = builtin(
    BuiltinId::MESH_PLANE,
    "mesh_plane",
    BuiltinTier::Tier2,
    &[
        req("w", F),
        req("d", F),
        super::Parameter::optional("columns", I, DefaultValue::Int(1)),
        super::Parameter::optional("rows", I, DefaultValue::Int(1)),
    ],
    MESH,
    Domain::Host,
    "meshPlane",
);

pub(super) const MESH_FROM: Builtin = builtin(
    BuiltinId::MESH_FROM,
    "mesh_from",
    BuiltinTier::Tier2,
    &[
        req("vertices", BuiltinType::FloatArray),
        req("indices", BuiltinType::IntArray),
    ],
    MESH,
    Domain::Host,
    "meshFrom",
);

pub(super) const NODE_ADD: Builtin = builtin(
    BuiltinId::NODE_ADD,
    "node_add",
    BuiltinTier::Tier2,
    &[req("mesh", MESH), req("material", MATERIAL)],
    NODE,
    Domain::Host,
    "nodeAdd",
);

pub(super) const NODE_SET_POS: Builtin = builtin(
    BuiltinId::NODE_SET_POS,
    "node_set_pos",
    BuiltinTier::Tier2,
    &[req("node", NODE), req("x", F), req("y", F), req("z", F)],
    V,
    Domain::Host,
    "nodeSetPos",
);

pub(super) const NODE_SET_ROT: Builtin = builtin(
    BuiltinId::NODE_SET_ROT,
    "node_set_rot",
    BuiltinTier::Tier2,
    &[req("node", NODE), req("x", F), req("y", F), req("z", F)],
    V,
    Domain::Host,
    "nodeSetRot",
);

pub(super) const NODE_SET_SCALE: Builtin = builtin(
    BuiltinId::NODE_SET_SCALE,
    "node_set_scale",
    BuiltinTier::Tier2,
    &[req("node", NODE), req("x", F), req("y", F), req("z", F)],
    V,
    Domain::Host,
    "nodeSetScale",
);

pub(super) const CAMERA_PERSPECTIVE: Builtin = builtin(
    BuiltinId::CAMERA_PERSPECTIVE,
    "camera_perspective",
    BuiltinTier::Tier2,
    &[req("fov", F), req("near", F), req("far", F)],
    V,
    Domain::Host,
    "cameraPerspective",
);

pub(super) const CAMERA_LOOK_AT: Builtin = builtin(
    BuiltinId::CAMERA_LOOK_AT,
    "camera_look_at",
    BuiltinTier::Tier2,
    &[
        req("eye", BuiltinType::Vec3),
        req("target", BuiltinType::Vec3),
        req("up", BuiltinType::Vec3),
    ],
    V,
    Domain::Host,
    "cameraLookAt",
);

pub(super) const LIGHT_DIRECTIONAL: Builtin = builtin(
    BuiltinId::LIGHT_DIRECTIONAL,
    "light_directional",
    BuiltinTier::Tier2,
    &[
        req("direction", BuiltinType::Vec3),
        req("color", BuiltinType::Vec3),
    ],
    V,
    Domain::Host,
    "lightDirectional",
);

pub(super) const TEXTURE_LOAD: Builtin = builtin(
    BuiltinId::TEXTURE_LOAD,
    "texture_load",
    BuiltinTier::Tier2,
    &[req("path", S)],
    TEXTURE,
    Domain::Host,
    "textureLoad",
);

pub(super) const SHADER_SET: Builtin = builtin(
    BuiltinId::SHADER_SET,
    "shader_set",
    BuiltinTier::Tier2,
    &[
        req("node", NODE),
        req("name", S),
        req("value", BuiltinType::ShaderValue),
    ],
    V,
    Domain::Host,
    "shaderSet",
);

pub(super) const SAMPLE: Builtin = builtin(
    BuiltinId::SAMPLE,
    "sample",
    BuiltinTier::Tier2,
    &[req("texture", TEXTURE), req("uv", BuiltinType::Vec2)],
    BuiltinType::Vec4,
    Domain::Gpu,
    "sampleTexture",
);

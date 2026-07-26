mod core;
mod drawing;
mod environment;
mod materials;
mod structures;

use crate::{
    Builtin, BuiltinId, BuiltinTier, BuiltinType, DefaultValue, Domain, Parameter, RuntimeOp,
    Signature,
};

const I: BuiltinType = BuiltinType::Int;
const F: BuiltinType = BuiltinType::Float;
const B: BuiltinType = BuiltinType::Bool;
const S: BuiltinType = BuiltinType::Str;
const V: BuiltinType = BuiltinType::Void;

const fn req(name: &'static str, ty: BuiltinType) -> Parameter {
    Parameter::required(name, ty)
}

const fn alpha() -> Parameter {
    Parameter::optional("a", F, DefaultValue::Float(1.0))
}

const fn builtin(
    id: BuiltinId,
    name: &'static str,
    tier: BuiltinTier,
    params: &'static [Parameter],
    result: BuiltinType,
    domain: Domain,
    runtime_op: &'static str,
) -> Builtin {
    Builtin {
        id,
        name,
        tier,
        signature: Signature { params, result },
        domain,
        runtime_op: RuntimeOp::new(runtime_op),
    }
}

pub(crate) static BUILTINS: &[Builtin] = &[
    core::FLOOR,
    core::ROUND,
    core::TRUNC,
    drawing::SIZE,
    drawing::BACKGROUND,
    drawing::FILL,
    drawing::STROKE,
    drawing::NO_STROKE,
    drawing::RECT,
    drawing::CIRCLE,
    drawing::LINE,
    drawing::TRIANGLE,
    drawing::TEXT,
    environment::PUSH_MATRIX,
    environment::POP_MATRIX,
    environment::TRANSLATE,
    environment::ROTATE,
    environment::SCALE,
    environment::WIDTH,
    environment::HEIGHT,
    environment::TIME,
    environment::MOUSE_X,
    environment::MOUSE_Y,
    environment::KEY_DOWN,
    environment::RANDOM,
    materials::MATERIAL_SHADER,
];

pub(crate) static BUILTIN_STRUCTS: &[crate::BuiltinStruct] = &[structures::EVENT];

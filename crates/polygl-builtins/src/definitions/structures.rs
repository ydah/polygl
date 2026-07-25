use crate::{BuiltinField, BuiltinStruct, BuiltinType, BuiltinValueType};

const fn scalar(ty: BuiltinType) -> BuiltinValueType {
    BuiltinValueType::Scalar(ty)
}

pub(super) const EVENT: BuiltinStruct = BuiltinStruct {
    name: "Event",
    fields: &[
        BuiltinField {
            name: "kind",
            ty: scalar(BuiltinType::Str),
        },
        BuiltinField {
            name: "x",
            ty: scalar(BuiltinType::Float),
        },
        BuiltinField {
            name: "y",
            ty: scalar(BuiltinType::Float),
        },
        BuiltinField {
            name: "key",
            ty: BuiltinValueType::Option(BuiltinType::Str),
        },
    ],
};

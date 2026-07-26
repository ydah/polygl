use std::collections::HashSet;
use std::error::Error;
use std::fmt;

use crate::definitions::{BUILTIN_STRUCTS, BUILTINS};
use crate::{Builtin, BuiltinType, DefaultValue};
use polygl_adapter_api::BuiltinResolver;
use polygl_hir::BuiltinId;

pub struct BuiltinTable;

impl BuiltinTable {
    #[must_use]
    pub const fn all() -> &'static [Builtin] {
        BUILTINS
    }

    #[must_use]
    pub fn find(name: &str) -> Option<&'static Builtin> {
        BUILTINS.iter().find(|builtin| builtin.name == name)
    }

    #[must_use]
    pub const fn structs() -> &'static [crate::BuiltinStruct] {
        BUILTIN_STRUCTS
    }

    #[must_use]
    pub fn find_struct(name: &str) -> Option<&'static crate::BuiltinStruct> {
        BUILTIN_STRUCTS
            .iter()
            .find(|definition| definition.name == name)
    }

    pub fn validate() -> Result<(), BuiltinTableError> {
        let mut names = HashSet::new();
        let mut ids = HashSet::new();
        let mut operations = HashSet::new();
        for builtin in BUILTINS {
            if !is_identifier(builtin.name) {
                return Err(BuiltinTableError::InvalidName(builtin.name));
            }
            if !names.insert(builtin.name) {
                return Err(BuiltinTableError::DuplicateName(builtin.name));
            }
            if !ids.insert(builtin.id) {
                return Err(BuiltinTableError::DuplicateId(builtin.name));
            }
            if !is_identifier(builtin.runtime_op.as_str()) {
                return Err(BuiltinTableError::InvalidRuntimeOp(
                    builtin.runtime_op.as_str(),
                ));
            }
            if !operations.insert(builtin.runtime_op.as_str()) {
                return Err(BuiltinTableError::DuplicateRuntimeOp(
                    builtin.runtime_op.as_str(),
                ));
            }
            validate_signature(builtin)?;
        }
        validate_structs()?;
        Ok(())
    }
}

impl BuiltinResolver for BuiltinTable {
    fn resolve_builtin(&self, canonical_name: &str) -> Option<BuiltinId> {
        Self::find(canonical_name).map(|builtin| builtin.id)
    }
}

fn validate_signature(builtin: &Builtin) -> Result<(), BuiltinTableError> {
    if builtin.signature.result == BuiltinType::ShaderValue {
        return Err(BuiltinTableError::ParameterOnlyResult(builtin.name));
    }
    let mut saw_optional = false;
    let mut names = HashSet::new();
    for param in builtin.signature.params {
        if !is_identifier(param.name) {
            return Err(BuiltinTableError::InvalidParameter(
                builtin.name,
                param.name,
            ));
        }
        if !names.insert(param.name) {
            return Err(BuiltinTableError::DuplicateParameter(
                builtin.name,
                param.name,
            ));
        }
        if param.ty == BuiltinType::Void {
            return Err(BuiltinTableError::VoidParameter(builtin.name, param.name));
        }
        if param.default.is_some() {
            saw_optional = true;
        } else if saw_optional {
            return Err(BuiltinTableError::RequiredAfterOptional(
                builtin.name,
                param.name,
            ));
        }
        if let Some(default) = param.default
            && !default_matches(default, param.ty)
        {
            return Err(BuiltinTableError::DefaultType(builtin.name, param.name));
        }
    }
    Ok(())
}

fn validate_structs() -> Result<(), BuiltinTableError> {
    let mut names = HashSet::new();
    for definition in BUILTIN_STRUCTS {
        if !is_type_identifier(definition.name) {
            return Err(BuiltinTableError::InvalidStructName(definition.name));
        }
        if !names.insert(definition.name) {
            return Err(BuiltinTableError::DuplicateStructName(definition.name));
        }
        let mut fields = HashSet::new();
        for field in definition.fields {
            if !is_identifier(field.name) {
                return Err(BuiltinTableError::InvalidStructField(
                    definition.name,
                    field.name,
                ));
            }
            if !fields.insert(field.name) {
                return Err(BuiltinTableError::DuplicateStructField(
                    definition.name,
                    field.name,
                ));
            }
            let scalar = match field.ty {
                crate::BuiltinValueType::Scalar(scalar)
                | crate::BuiltinValueType::Option(scalar) => scalar,
            };
            if scalar == BuiltinType::Void {
                return Err(BuiltinTableError::VoidStructField(
                    definition.name,
                    field.name,
                ));
            }
        }
    }
    Ok(())
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

fn is_type_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|first| first.is_ascii_uppercase())
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_')
}

const fn default_matches(default: DefaultValue, ty: BuiltinType) -> bool {
    matches!(
        (default, ty),
        (DefaultValue::Int(_), BuiltinType::Int)
            | (DefaultValue::Float(_), BuiltinType::Float)
            | (DefaultValue::Bool(_), BuiltinType::Bool)
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinTableError {
    InvalidName(&'static str),
    DuplicateName(&'static str),
    DuplicateId(&'static str),
    InvalidRuntimeOp(&'static str),
    DuplicateRuntimeOp(&'static str),
    InvalidParameter(&'static str, &'static str),
    DuplicateParameter(&'static str, &'static str),
    VoidParameter(&'static str, &'static str),
    RequiredAfterOptional(&'static str, &'static str),
    DefaultType(&'static str, &'static str),
    InvalidStructName(&'static str),
    DuplicateStructName(&'static str),
    InvalidStructField(&'static str, &'static str),
    DuplicateStructField(&'static str, &'static str),
    VoidStructField(&'static str, &'static str),
    ParameterOnlyResult(&'static str),
}

impl fmt::Display for BuiltinTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(formatter, "invalid builtin name `{name}`"),
            Self::DuplicateName(name) => write!(formatter, "duplicate builtin name `{name}`"),
            Self::DuplicateId(name) => write!(formatter, "`{name}` reuses a builtin identifier"),
            Self::InvalidRuntimeOp(name) => {
                write!(formatter, "invalid runtime operation `{name}`")
            }
            Self::DuplicateRuntimeOp(name) => {
                write!(formatter, "duplicate runtime operation `{name}`")
            }
            Self::InvalidParameter(builtin, param) => {
                write!(
                    formatter,
                    "`{builtin}` has invalid parameter name `{param}`"
                )
            }
            Self::DuplicateParameter(builtin, param) => {
                write!(formatter, "`{builtin}` repeats parameter `{param}`")
            }
            Self::VoidParameter(builtin, param) => {
                write!(formatter, "`{builtin}` parameter `{param}` has type void")
            }
            Self::RequiredAfterOptional(builtin, param) => write!(
                formatter,
                "`{builtin}` required parameter `{param}` follows an optional parameter"
            ),
            Self::DefaultType(builtin, param) => {
                write!(
                    formatter,
                    "`{builtin}` parameter `{param}` has a mistyped default"
                )
            }
            Self::InvalidStructName(name) => write!(formatter, "invalid builtin struct `{name}`"),
            Self::DuplicateStructName(name) => {
                write!(formatter, "duplicate builtin struct `{name}`")
            }
            Self::InvalidStructField(structure, field) => {
                write!(formatter, "`{structure}` has invalid field `{field}`")
            }
            Self::DuplicateStructField(structure, field) => {
                write!(formatter, "`{structure}` repeats field `{field}`")
            }
            Self::VoidStructField(structure, field) => {
                write!(formatter, "`{structure}` field `{field}` has type void")
            }
            Self::ParameterOnlyResult(name) => {
                write!(formatter, "`{name}` returns a parameter-only builtin type")
            }
        }
    }
}

impl Error for BuiltinTableError {}

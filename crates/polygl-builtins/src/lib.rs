//! Canonical builtin metadata shared by compiler analysis and orchestration.

/// Compatibility version of builtin names, signatures, and runtime operations.
pub const BUILTIN_SCHEMA_VERSION: u32 = 2;

mod builtin;
mod definitions;
mod table;
#[cfg(test)]
mod table_tests;

pub use builtin::{
    Builtin, BuiltinField, BuiltinStruct, BuiltinTier, BuiltinType, BuiltinValueType, DefaultValue,
    Domain, Parameter, RuntimeOp, Signature,
};
pub use polygl_hir::BuiltinId;
pub use table::{BuiltinTable, BuiltinTableError};

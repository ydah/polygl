//! Canonical builtin metadata shared by compiler analysis and orchestration.

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

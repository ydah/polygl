//! Compiler pipeline primitives and the canonical builtin registry.

mod builtin;
mod definitions;
mod table;
#[cfg(test)]
mod table_tests;

pub use builtin::{
    Builtin, BuiltinTier, BuiltinType, DefaultValue, Domain, Parameter, RuntimeOp, Signature,
};
pub use polygl_hir::BuiltinId;
pub use table::{BuiltinTable, BuiltinTableError};

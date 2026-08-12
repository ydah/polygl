//! Structured, typed low-level intermediate representation.

/// Serialization and tooling contract version for the low-level IR schema.
pub const LIR_SCHEMA_VERSION: u32 = 1;

mod domain;
mod expr;
mod lower;
mod module;
mod optimize;
mod split;
mod stmt;

pub use expr::{BinaryOp, CallTarget, Expr, ExprKind, FieldInit, Literal, MapEntry, UnaryOp};
pub use lower::lower;
pub use module::{
    Constant, Domain, EntryKind, EntryPoint, Field, Function, Module, Parameter, StructDef,
};
pub use split::{AssetReference, SplitProgram, split};
pub use stmt::{Block, Place, PlaceKind, Range, Statement, StatementKind};

#[cfg(test)]
mod tests;

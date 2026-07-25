//! Structured, typed low-level intermediate representation.

mod domain;
mod expr;
mod lower;
mod module;
mod optimize;
mod stmt;

pub use expr::{BinaryOp, CallTarget, Expr, ExprKind, FieldInit, Literal, MapEntry, UnaryOp};
pub use lower::lower;
pub use module::{
    Constant, Domain, EntryKind, EntryPoint, Field, Function, Module, Parameter, StructDef,
};
pub use stmt::{Block, Place, PlaceKind, Range, Statement, StatementKind};

#[cfg(test)]
mod tests;

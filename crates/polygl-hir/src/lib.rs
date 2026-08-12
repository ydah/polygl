//! PolyGL's source-oriented, language-independent high-level IR.

/// Schema version for tooling that persists or exchanges HIR.
pub const HIR_SCHEMA_VERSION: u32 = 1;

mod builder;
mod dump;
mod expr;
mod item;
mod metrics;
mod normalize;
mod stmt;
mod symbol;
mod types;

pub use builder::HirBuilder;
pub use dump::{dump, normalized_dump};
pub use expr::{BinOp, Callee, Expr, ExprKind, FieldInit, Literal, MapEntry, UnOp};
pub use item::{
    ConstDef, DomainHint, EntryPoint, EntryPointKind, FieldDef, Function, Item, Module, Param,
    StructDef,
};
pub use metrics::{ModuleMetrics, module_metrics};
pub use stmt::{Block, Place, PlaceKind, RangeExpr, Stmt, StmtKind};
pub use symbol::{BuiltinId, Symbol};
pub use types::{OpaqueType, TypeExpr, TypeKind};

#[cfg(test)]
mod tests;

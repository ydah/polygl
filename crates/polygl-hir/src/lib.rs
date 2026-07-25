//! PolyGL's source-oriented, language-independent high-level IR.

mod builder;
mod dump;
mod expr;
mod item;
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
pub use stmt::{Block, Place, PlaceKind, RangeExpr, Stmt, StmtKind};
pub use symbol::{BuiltinId, Symbol};
pub use types::{OpaqueType, TypeExpr, TypeKind};

#[cfg(test)]
mod tests;

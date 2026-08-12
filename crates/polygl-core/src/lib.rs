//! Public compiler pipeline orchestration and compatibility re-exports.

mod pass;
mod pipeline;
mod registry;
mod stage;

pub use pass::{CompileStage, PassTrace};
pub use pipeline::{CompileError, CompileOptions, CompileOutput, Compiler, FrontendOutput};
pub use polygl_backend_js::{BuildMode, SourceMapMode};
pub use registry::{AdapterRegistry, RegistryError};
pub use stage::{
    CompileBudget, CompileStatistics, DomainResolvedLir, LoweredHir, TypedHir,
    ValidatedSplitProgram,
};

pub use polygl_builtins::BUILTIN_SCHEMA_VERSION;
pub use polygl_builtins::{
    Builtin, BuiltinField, BuiltinId, BuiltinStruct, BuiltinTable, BuiltinTableError, BuiltinTier,
    BuiltinType, BuiltinValueType, DefaultValue, Domain, Parameter, RuntimeOp, Signature,
};

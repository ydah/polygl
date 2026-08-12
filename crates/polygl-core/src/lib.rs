//! Public compiler pipeline orchestration and compatibility re-exports.

mod pipeline;
mod registry;

pub use pipeline::{CompileError, CompileOptions, CompileOutput, Compiler, FrontendOutput};
pub use polygl_backend_js::{BuildMode, SourceMapMode};
pub use registry::{AdapterRegistry, RegistryError};

pub use polygl_builtins::BUILTIN_SCHEMA_VERSION;
pub use polygl_builtins::{
    Builtin, BuiltinField, BuiltinId, BuiltinStruct, BuiltinTable, BuiltinTableError, BuiltinTier,
    BuiltinType, BuiltinValueType, DefaultValue, Domain, Parameter, RuntimeOp, Signature,
};

//! GLSL ES 3.00 code generation for validated GPU LIR.

/// Compatibility version of generated shader metadata and runtime bindings.
pub const SHADER_ABI_VERSION: u32 = 1;

mod backend;
mod emitter;
mod error;
mod model;

pub use backend::GlslBackend;
pub use error::EmitError;
pub use model::{
    AttributeBinding, GlslArtifacts, ShaderArtifact, ShaderStage, UniformBinding, UniformSource,
};

#[cfg(test)]
mod tests;

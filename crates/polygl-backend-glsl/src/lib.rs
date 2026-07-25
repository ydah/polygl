//! GLSL ES 3.00 code generation for validated GPU LIR.

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

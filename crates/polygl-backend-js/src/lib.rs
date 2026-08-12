//! ES2020 code generation with Source Map v3 and optional runtime checks.

mod backend;
mod emitter;
mod error;
mod source_map;

pub use backend::{Artifacts, Backend, BuildMode, JavaScriptBackend, SourceMapMode};
pub use error::EmitError;

#[cfg(test)]
mod tests;

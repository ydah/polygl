//! ES2020 code generation with Source Map v3 and optional runtime checks.

mod backend;
mod emitter;
mod error;
mod source_map;

pub use backend::{Artifacts, Backend, BuildMode, JavaScriptBackend, SourceMapMode};
pub use error::EmitError;

/// Compatibility version shared by generated programs and the browser runtime.
pub const RUNTIME_ABI_VERSION: u32 = 1;

#[cfg(test)]
mod tests;

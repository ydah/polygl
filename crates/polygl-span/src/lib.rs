//! Source locations and structured diagnostics.

mod collection;
mod diagnostic;
mod render_error;
mod source;
mod span;

pub use collection::Diagnostics;
pub use diagnostic::{Diagnostic, Label, Severity, Suggestion};
pub use render_error::RenderError;
pub use source::{SourceError, SourceFile, SourceId, SourcePosition};
pub use span::{Span, SpanError};

#[cfg(test)]
mod diagnostic_tests;

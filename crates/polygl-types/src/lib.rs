//! Type inference and call-site monomorphization.

mod analyzer;
mod solver;
mod ty;
mod typed;

pub use analyzer::{AnalyzeOptions, analyze, analyze_with_options};
pub use ty::Type;
pub use typed::TypedModule;

#[cfg(test)]
mod tests;

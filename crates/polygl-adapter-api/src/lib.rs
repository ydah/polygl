//! Public language adapter API.

mod common;
mod feature;
mod lower;

pub use common::{
    automatic_uniform_type, canonical_entry_kind, constructor_function_name,
    is_portable_identifier, parse_annotation_type, vector_constructor_size,
};
pub use feature::FeatureTag;
pub use lower::{BuiltinResolver, LanguageAdapter, LowerCtx};

/// Compatibility version of the public adapter trait and lowering contract.
pub const ADAPTER_API_VERSION: u32 = 1;

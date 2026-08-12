//! Shared selection and comparison machinery for all conformance layers.

mod adapter_corpus;
mod case;
mod error;
mod frame;
mod semantic;
mod smoke;
mod snapshot;

pub use adapter_corpus::verify_adapter_corpus;
pub use case::{
    ConformanceCase, ConformanceLanguage, ConformanceLayer, load_manifest, select_cases,
};
pub use error::ConformanceError;
pub use frame::{L1BaselineStore, RenderedFrame, compare_frames};
pub use semantic::verify_host_semantics;
pub use smoke::{ConformanceReport, verify_smoke};
pub use snapshot::{L2SnapshotStore, L3SnapshotStore, NeutralProgram, compare_neutral_hir};

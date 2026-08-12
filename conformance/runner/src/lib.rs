//! Shared selection and comparison machinery for all conformance layers.

mod case;
mod error;
mod frame;
mod smoke;
mod snapshot;

pub use case::{
    ConformanceCase, ConformanceLanguage, ConformanceLayer, load_manifest, select_cases,
};
pub use error::ConformanceError;
pub use frame::{L1BaselineStore, RenderedFrame, compare_frames};
pub use smoke::{ConformanceReport, verify_smoke};
pub use snapshot::{L2SnapshotStore, L3SnapshotStore, NeutralProgram, compare_neutral_hir};

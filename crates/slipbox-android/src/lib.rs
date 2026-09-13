//! Android native consumer of the headless slipbox engine.
//!
//! The crate links the canonical `slipbox-engine` and its bundled SQLite into
//! one shared library and exports a single JNI entry point that runs a
//! self-contained fixture probe. It is not the versioned reader adapter: it
//! exposes no note-reading or index-maintenance operation to Kotlin.

pub mod jni_seam;
pub mod probe;

pub use probe::{ProbeCheck, ProbeFailure, ProbeReport, run_fixture_probe};

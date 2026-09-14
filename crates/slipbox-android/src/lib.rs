//! Android native consumer of the headless slipbox engine.
//!
//! The crate links the canonical `slipbox-engine` and its bundled SQLite into
//! one shared library and exports a closed set of JNI entry points: the
//! versioned adapter over owned reading and index-maintenance sessions, and the
//! self-contained fixture probe that qualifies the packaged engine on a device.

pub mod adapter;
pub mod jni_seam;
pub mod probe;

pub use probe::{ProbeCheck, ProbeFailure, ProbeReport, run_fixture_probe};

//! Headless slipbox service.

mod dispatch;
mod handlers;
mod occurrences_query;
mod reflinks_query;
mod rpc;
mod state;
mod text_query;
mod unlinked_references_query;
mod workflows;

pub mod operations;
pub mod root_path;
pub mod service;

pub use dispatch::handle_request;
pub use slipbox_index::{DiscoveryPolicy, PlatformPolicy};

/// Index-backed queries shared by service handlers and benchmarks.
pub mod queries {
    pub use crate::occurrences_query::query_occurrences;
    pub use crate::reflinks_query::query_reflinks;
    pub use crate::unlinked_references_query::query_unlinked_references;
}

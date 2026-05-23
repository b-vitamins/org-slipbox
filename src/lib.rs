//! Reusable Rust substrate for slipbox adapters.
//!
//! The CLI binary, JSON-RPC daemon, benchmark harness, and future protocol
//! adapters all depend on the same parsing, indexing, query, and write code.

mod occurrences_query;
mod reflinks_query;
pub mod server;
pub mod service;
mod text_query;
mod unlinked_references_query;

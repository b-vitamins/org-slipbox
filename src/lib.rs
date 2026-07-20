//! Reusable Rust substrate for slipbox front-ends.
//!
//! The CLI binary, JSON-RPC daemon, and benchmark harness all depend on the
//! same parsing, indexing, query, and write code.

mod occurrences_query;
mod reflinks_query;
pub mod root_path;
pub mod server;
pub mod service;
mod text_query;
mod unlinked_references_query;

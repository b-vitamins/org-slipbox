//! The read-only HTTP reading surface over a [`ReadingBridge`](crate::ReadingBridge).
//!
//! This layer maps a strict allowlist of `GET /api/...` routes to the bridge's
//! read methods, serializes `slipbox-core` result types verbatim as JSON, and
//! maps every failure to an HTTP status through a single error envelope. Every
//! other path is served from the embedded reading client, so one port answers
//! both the API and the app.

mod query;
mod response;
mod routes;
mod server;
mod wire;

pub use server::ReadingServer;

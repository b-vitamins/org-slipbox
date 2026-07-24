//! The read-only HTTP reading API over a [`ReadingBridge`](crate::ReadingBridge).
//!
//! This layer maps a small set of `GET /api/...` routes to the bridge's read
//! methods, serializes `slipbox-core` result types verbatim as JSON, and maps
//! every failure to an HTTP status through a single error envelope. It adds no
//! capability of its own: the read-only guarantee is the bridge's, enforced
//! again by the daemon's dispatch guard, and the routing table is itself a
//! strict allowlist of read methods.

mod neighborhood;
mod query;
mod response;
mod routes;
mod server;
mod wire;

pub use server::ReadingServer;

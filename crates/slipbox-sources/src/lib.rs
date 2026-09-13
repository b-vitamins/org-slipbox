//! Configured repository sources and the one active selection among them.
//!
//! Domain types live in `slipbox-core`. Catalog values are caller-owned.

mod catalog;
mod store;

pub use catalog::*;
pub use store::*;

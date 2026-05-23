#[path = "../occurrences_query.rs"]
mod occurrences_query;
#[path = "../reflinks_query.rs"]
mod reflinks_query;
mod root_path {
    pub use slipbox::root_path::resolve_root_path_from_canonical_root;
}
#[path = "../text_query.rs"]
mod text_query;
#[path = "../unlinked_references_query.rs"]
mod unlinked_references_query;

mod slipbox_bench;

fn main() -> anyhow::Result<()> {
    slipbox_bench::main()
}

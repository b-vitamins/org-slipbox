#[path = "../occurrences_query.rs"]
mod occurrences_query;
#[path = "../reflinks_query.rs"]
mod reflinks_query;
#[allow(dead_code)]
#[path = "../server/mod.rs"]
mod server;
#[path = "../text_query.rs"]
mod text_query;
#[path = "../unlinked_references_query.rs"]
mod unlinked_references_query;

mod slipbox_bench;

fn main() -> anyhow::Result<()> {
    slipbox_bench::main()
}

mod occurrences_query;
mod reflinks_query;
mod server;
mod text_query;
mod unlinked_references_query;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use slipbox_index::DiscoveryPolicy;

#[derive(Debug, Parser)]
#[command(author, version, about = "Org slipbox tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the JSON-RPC daemon over stdio.
    Serve {
        /// Root directory containing Org files.
        #[arg(long)]
        root: PathBuf,
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
        /// File extensions eligible for discovery and indexing.
        #[arg(long = "file-extension")]
        file_extensions: Vec<String>,
        /// Relative-path regular expressions to exclude from discovery.
        #[arg(long = "exclude-regexp")]
        exclude_regexps: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve {
            root,
            db,
            file_extensions,
            exclude_regexps,
        } => {
            let discovery = if file_extensions.is_empty() && exclude_regexps.is_empty() {
                DiscoveryPolicy::default()
            } else {
                DiscoveryPolicy::new(file_extensions, exclude_regexps)?
            };
            server::serve(root, db, discovery)
        }
    }
}

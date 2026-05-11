mod cli;
mod occurrences_query;
mod reflinks_query;
mod server;
mod text_query;
mod unlinked_references_query;

use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about = "Org slipbox tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the JSON-RPC daemon over stdio.
    Serve(ServeArgs),
    /// Show daemon status over the canonical headless connection path.
    Status(cli::StatusArgs),
    /// Resolve an exact note target over the canonical headless connection path.
    ResolveNode(cli::ResolveNodeArgs),
    /// Run live declared-lens exploration over the canonical headless connection path.
    Explore(cli::ExploreArgs),
    /// Compare two resolved notes over the canonical headless connection path.
    Compare(cli::CompareArgs),
    /// Run corpus-health audits over the canonical headless connection path.
    Audit(cli::AuditArgs),
    /// Discover, inspect, and run named workflows over the canonical headless connection path.
    Workflow(cli::WorkflowArgs),
    /// Manage durable exploration artifacts over the canonical headless connection path.
    Artifact(cli::ArtifactArgs),
    /// Manage declarative workbench packs over the canonical headless connection path.
    Pack(cli::PackArgs),
    /// Inspect and manage durable operational review runs over the canonical headless connection path.
    Review(cli::ReviewArgs),
}

#[derive(Debug, Args)]
struct ServeArgs {
    #[command(flatten)]
    scope: cli::ScopeArgs,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => cli::report_error(&error),
    }
}

fn run() -> Result<(), cli::CliCommandError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => run_serve(args)
            .map_err(|error| cli::CliCommandError::new(cli::OutputMode::Human, error)),
        Command::Status(args) => cli::run_status(&args),
        Command::ResolveNode(args) => cli::run_resolve_node(&args),
        Command::Explore(args) => cli::run_explore(&args),
        Command::Compare(args) => cli::run_compare(&args),
        Command::Audit(args) => cli::run_audit(&args),
        Command::Workflow(args) => cli::run_workflow(&args),
        Command::Artifact(args) => cli::run_artifact(&args),
        Command::Pack(args) => cli::run_pack(&args),
        Command::Review(args) => cli::run_review(&args),
    }
}

fn run_serve(args: ServeArgs) -> Result<()> {
    let discovery = args.scope.discovery_policy()?;
    server::serve(
        args.scope.root,
        args.scope.db,
        args.scope.workflow_dirs,
        discovery,
    )
}

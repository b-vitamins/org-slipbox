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
    /// Refresh indexed files over the canonical headless connection path.
    Sync(cli::SyncArgs),
    /// List and search indexed files over the canonical headless connection path.
    File(cli::FileArgs),
    /// Inspect nodes and node neighborhoods over the canonical headless connection path.
    Node(cli::NodeArgs),
    /// Search and resolve indexed references over the canonical headless connection path.
    Ref(cli::RefArgs),
    /// Search indexed tags over the canonical headless connection path.
    Tag(cli::TagArgs),
    /// Search indexed note text over the canonical headless connection path.
    Search(cli::SearchArgs),
    /// Query agenda entries over the canonical headless connection path.
    Agenda(cli::AgendaArgs),
    /// Create and append daily notes over the canonical headless connection path.
    Daily(cli::DailyArgs),
    /// Export graph DOT over the canonical headless connection path.
    Graph(cli::GraphArgs),
    /// Run capture operations over the canonical headless connection path.
    Capture(cli::CaptureArgs),
    /// Create and append notes over the canonical headless connection path.
    Note(cli::NoteArgs),
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
    /// Discover, inspect, and run review routines over the canonical headless connection path.
    Routine(cli::RoutineArgs),
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
        Command::Sync(args) => cli::run_sync(&args),
        Command::File(args) => cli::run_file(&args),
        Command::Node(args) => cli::run_node(&args),
        Command::Ref(args) => cli::run_ref(&args),
        Command::Tag(args) => cli::run_tag(&args),
        Command::Search(args) => cli::run_search(&args),
        Command::Agenda(args) => cli::run_agenda(&args),
        Command::Daily(args) => cli::run_daily(&args),
        Command::Graph(args) => cli::run_graph(&args),
        Command::Capture(args) => cli::run_capture(&args),
        Command::Note(args) => cli::run_note(&args),
        Command::ResolveNode(args) => cli::run_resolve_node(&args),
        Command::Explore(args) => cli::run_explore(&args),
        Command::Compare(args) => cli::run_compare(&args),
        Command::Audit(args) => cli::run_audit(&args),
        Command::Workflow(args) => cli::run_workflow(&args),
        Command::Routine(args) => cli::run_routine(&args),
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

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
#[command(
    author,
    version,
    about = "Knowledge management using Org",
    long_about = "Knowledge management using Org.

slipbox indexes and edits an Org directory through a Rust daemon. Org files remain the source of truth; SQLite is a derived index. Most commands spawn `slipbox serve` over JSON-RPC stdio using --root and --db, then exit after the requested operation.",
    after_help = "Command families:
  Notes:       node, note, capture, daily, edit, resolve-node
  Relations:   ref, tag, search, agenda, graph, link
  Exploration: explore, compare, artifact
  Reviews:     audit, review
  Assets:      workflow, routine, pack
  System:      serve, status, sync, file, diagnose

Use --json on daemon-backed commands for stable machine output. Local inspection commands such as `workflow show --spec` and `pack validate` say so in their own help."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the JSON-RPC daemon over stdio.
    #[command(
        long_about = "Run the JSON-RPC daemon over stdio for daemon-backed commands and editor clients."
    )]
    Serve(ServeArgs),
    /// Show daemon and index status.
    #[command(
        long_about = "Show daemon and derived-index status. This is a read-only daemon-backed command; use --json for the stable machine envelope."
    )]
    Status(cli::StatusArgs),
    /// Refresh derived index state from Org files.
    #[command(
        long_about = "Refresh derived index state from Org files. Root sync applies discovery rules to the tree; file sync refreshes one file without pruning unrelated index rows."
    )]
    Sync(cli::SyncArgs),
    /// List and search indexed Org files.
    #[command(
        long_about = "List and search indexed Org files from the derived index. These commands are read-only and report canonical file records with --json."
    )]
    File(cli::FileArgs),
    /// Explain file, node, and index health.
    #[command(
        long_about = "Explain file, node, and index health. Diagnostics are read-only and describe discovery eligibility, indexed state, and source-location consistency."
    )]
    Diagnose(cli::DiagnoseArgs),
    /// Inspect notes, anchors, metadata, and note neighborhoods.
    #[command(
        long_about = "Inspect and mutate note records and anchor identity. Read commands return indexed records; write commands mutate Org files through the daemon and refresh affected index state before returning."
    )]
    Node(cli::NodeArgs),
    /// Search and resolve indexed refs.
    #[command(
        long_about = "Search and resolve indexed references. Refs are normalized during indexing; resolution returns the note that owns the exact ref."
    )]
    Ref(cli::RefArgs),
    /// Search indexed tags.
    #[command(
        long_about = "Search indexed tags. Tags are derived from Org metadata and heading tags."
    )]
    Tag(cli::TagArgs),
    /// Search indexed note text occurrences.
    #[command(
        long_about = "Search raw indexed text occurrences. Results include the owning anchor and source location."
    )]
    Search(cli::SearchArgs),
    /// Query scheduled, deadline, and closed planning entries.
    #[command(
        long_about = "Query planning entries from indexed Org planning lines. `today` uses the local system date; `date` and `range` require ISO dates."
    )]
    Agenda(cli::AgendaArgs),
    /// Ensure, inspect, and append daily notes.
    #[command(
        long_about = "Ensure, inspect, and append daily notes. Daily paths are generated relative to --root, must stay inside the root, and must end in .org."
    )]
    Daily(cli::DailyArgs),
    /// Export relation graph DOT.
    #[command(
        long_about = "Export relation graph DOT. This command does not run Graphviz; it emits DOT to stdout or writes it to --output."
    )]
    Graph(cli::GraphArgs),
    /// Run capture-template operations.
    #[command(
        long_about = "Run capture-template operations. `template` mutates Org files through the daemon and refreshes affected index state; `preview` renders the daemon-owned capture result without writing files."
    )]
    Capture(cli::CaptureArgs),
    /// Create file notes and append headings.
    #[command(
        long_about = "Create file notes and append headings. This family mutates Org files through the daemon, then refreshes affected files before returning so follow-up reads observe the write."
    )]
    Note(cli::NoteArgs),
    /// Move or rewrite Org structure through daemon-owned edits.
    #[command(
        long_about = "Move or rewrite Org structure through daemon-owned edits. Every edit reports changed/removed files and refreshed-index status; use --json for the stable StructuralWriteReport."
    )]
    Edit(cli::EditArgs),
    /// Inspect and rewrite supported Org links.
    #[command(
        long_about = "Inspect and rewrite supported Org link forms. Preview commands are read-only; apply commands require explicit confirmation and report refreshed affected files."
    )]
    Link(cli::LinkArgs),
    /// Resolve an exact note target.
    #[command(
        long_about = "Resolve an exact note target to the indexed note record selected by --id, --title, --ref, or --key."
    )]
    ResolveNode(cli::ResolveNodeArgs),
    /// Run a live exploration lens.
    #[command(
        long_about = "Run a live exploration lens from one focus. `--key` may target an anchor for anchor-aware lenses; other target selectors resolve notes. Use --save with artifact metadata to persist the live result."
    )]
    Explore(cli::ExploreArgs),
    /// Compare two resolved notes.
    #[command(
        long_about = "Compare two exact note targets through the daemon. The JSON output is the canonical comparison result; --group filters the returned sections without changing comparison semantics. Use --save with artifact metadata to persist the comparison."
    )]
    Compare(cli::CompareArgs),
    /// Run corpus-health audits.
    #[command(
        long_about = "Run corpus-health audits over the derived index. Audits are read-only unless --save-review is set; report output can be human, JSON, JSONL, or written to a file."
    )]
    Audit(cli::AuditArgs),
    /// Discover, inspect, and run named workflows.
    #[command(
        long_about = "Discover, inspect, and run named workflows. Built-in and discovered workflows run through the daemon; `workflow show --spec` is local JSON inspection and does not require --root or --db."
    )]
    Workflow(cli::WorkflowArgs),
    /// Discover, inspect, and run review routines.
    #[command(
        long_about = "List, inspect, and run review routines. Routines compose audits or workflows and may save durable review runs through daemon-owned semantics."
    )]
    Routine(cli::RoutineArgs),
    /// Manage durable exploration artifacts.
    #[command(
        long_about = "Manage durable exploration artifacts. Artifacts are explicit JSON records stored beside the database and executed through live daemon semantics."
    )]
    Artifact(cli::ArtifactArgs),
    /// Manage declarative workbench packs.
    #[command(
        long_about = "Manage declarative workbench packs. `pack validate` is local file/stdin inspection; import, export, list, show, and delete use the daemon and durable pack store."
    )]
    Pack(cli::PackArgs),
    /// Inspect and manage durable operational review runs.
    #[command(
        long_about = "Inspect, diff, mark, delete, and remediate durable review runs. Remediation preview is read-only; apply requires explicit confirmation and reports changed-file refresh status."
    )]
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
        Command::Diagnose(args) => cli::run_diagnose(&args),
        Command::Node(args) => cli::run_node(&args),
        Command::Ref(args) => cli::run_ref(&args),
        Command::Tag(args) => cli::run_tag(&args),
        Command::Search(args) => cli::run_search(&args),
        Command::Agenda(args) => cli::run_agenda(&args),
        Command::Daily(args) => cli::run_daily(&args),
        Command::Graph(args) => cli::run_graph(&args),
        Command::Capture(args) => cli::run_capture(&args),
        Command::Note(args) => cli::run_note(&args),
        Command::Edit(args) => cli::run_edit(&args),
        Command::Link(args) => cli::run_link(&args),
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

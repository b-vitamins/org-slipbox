use super::runtime::{
    CliCommandError, HeadlessArgs, HeadlessCommand, ResolveTargetArgs,
    normalize_diagnostic_file_path, resolve_anchor_or_note_target_key, run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    FileDiagnosticsParams, FileDiagnosticsResult, IndexDiagnosticsResult, IndexFileParams,
    IndexFileResult, IndexStats, IndexedFilesResult, NodeDiagnosticsParams, NodeDiagnosticsResult,
    SearchFilesParams, SearchFilesResult, StatusInfo,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::path::PathBuf;

use super::render::*;

#[derive(Debug, Clone, Args)]
pub(crate) struct StatusArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncArgs {
    #[command(subcommand)]
    pub(crate) command: SyncCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SyncCommand {
    /// Refresh the full indexed root through daemon-owned discovery.
    Root(SyncRootArgs),
    /// Refresh one file's indexed state without pruning the rest of the root.
    File(SyncFileArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncRootArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to refresh, absolute or relative to --root.
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileArgs {
    #[command(subcommand)]
    pub(crate) command: FileCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum FileCommand {
    /// List indexed files.
    List(FileListArgs),
    /// Search indexed file paths and titles.
    Search(FileSearchArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Search query matched against indexed file paths and titles.
    pub(crate) query: String,
    /// Maximum file records to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DiagnoseArgs {
    #[command(subcommand)]
    pub(crate) command: DiagnoseCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DiagnoseCommand {
    /// Diagnose one file's discovery eligibility and indexed state.
    File(DiagnoseFileArgs),
    /// Diagnose one indexed node's source file and line state.
    Node(DiagnoseNodeArgs),
    /// Diagnose eligible-vs-indexed file drift and status consistency.
    Index(DiagnoseIndexArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DiagnoseFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to diagnose, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DiagnoseNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DiagnoseIndexArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

pub(crate) fn run_status(args: &StatusArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn run_sync(args: &SyncArgs) -> Result<(), CliCommandError> {
    match &args.command {
        SyncCommand::Root(command) => run_headless_command(command),
        SyncCommand::File(command) => run_headless_command(command),
    }
}

pub(crate) fn run_file(args: &FileArgs) -> Result<(), CliCommandError> {
    match &args.command {
        FileCommand::List(command) => run_headless_command(command),
        FileCommand::Search(command) => run_headless_command(command),
    }
}

pub(crate) fn run_diagnose(args: &DiagnoseArgs) -> Result<(), CliCommandError> {
    match &args.command {
        DiagnoseCommand::File(command) => run_headless_command(command),
        DiagnoseCommand::Node(command) => run_headless_command(command),
        DiagnoseCommand::Index(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for StatusArgs {
    type Output = StatusInfo;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.status()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!(
            "version: {}\nroot: {}\ndb: {}\nfiles indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
            output.version,
            output.root,
            output.db,
            output.files_indexed,
            output.nodes_indexed,
            output.links_indexed,
        )
    }
}

impl HeadlessCommand for SyncRootArgs {
    type Output = IndexStats;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.index()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        let mut rendered = String::from("refreshed root\n");
        rendered.push_str(&render_index_stats(output));
        rendered
    }
}

impl HeadlessCommand for SyncFileArgs {
    type Output = IndexFileResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.index_file(&IndexFileParams {
            file_path: self.path.display().to_string(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("refreshed file: {}\n", output.file_path)
    }
}

impl HeadlessCommand for FileListArgs {
    type Output = IndexedFilesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.indexed_files()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_indexed_files(output)
    }
}

impl HeadlessCommand for FileSearchArgs {
    type Output = SearchFilesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_files(&SearchFilesParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_file_search_result(output)
    }
}

impl HeadlessCommand for DiagnoseFileArgs {
    type Output = FileDiagnosticsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let file_path = normalize_diagnostic_file_path(&self.headless.scope.root, &self.file)?;
        client.diagnose_file(&FileDiagnosticsParams { file_path })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_file_diagnostics(&output.diagnostic)
    }
}

impl HeadlessCommand for DiagnoseNodeArgs {
    type Output = NodeDiagnosticsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node_key = resolve_anchor_or_note_target_key(client, &self.target.target())?;
        client.diagnose_node(&NodeDiagnosticsParams { node_key })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_diagnostics(&output.diagnostic)
    }
}

impl HeadlessCommand for DiagnoseIndexArgs {
    type Output = IndexDiagnosticsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.diagnose_index()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_index_diagnostics(&output.diagnostic)
    }
}

use super::super::output::CliCommandError;
use super::super::render::{notes::render_node_summary, relations::render_ref_search_result};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, require_resolved_node, run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{NodeFromRefParams, NodeRecord, SearchRefsParams, SearchRefsResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct RefArgs {
    #[command(subcommand)]
    pub(crate) command: RefCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum RefCommand {
    /// Search indexed references.
    Search(RefSearchArgs),
    /// Resolve one reference to its note.
    Resolve(RefResolveArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RefSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Reference query.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum reference records to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RefResolveArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Reference to resolve, for example `cite:smith2026`.
    #[arg(value_name = "REF")]
    pub(crate) reference: String,
}

pub(crate) fn run_ref(args: &RefArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RefCommand::Search(command) => run_headless_command(command),
        RefCommand::Resolve(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for RefSearchArgs {
    type Output = SearchRefsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_refs(&SearchRefsParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_ref_search_result(output)
    }
}

impl HeadlessCommand for RefResolveArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        require_resolved_node(
            client.node_from_ref(&NodeFromRefParams {
                reference: self.reference.clone(),
            })?,
            format!("unknown node ref: {}", self.reference),
        )
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

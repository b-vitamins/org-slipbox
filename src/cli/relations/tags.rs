use super::super::output::CliCommandError;
use super::super::render::relations::render_tag_search_result;
use super::super::runtime::{HeadlessArgs, HeadlessCommand, run_headless_command};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{SearchTagsParams, SearchTagsResult};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct TagArgs {
    #[command(subcommand)]
    pub(crate) command: TagCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum TagCommand {
    /// Search indexed tags.
    Search(TagSearchArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct TagSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Tag query.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum tags to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

pub(crate) fn run_tag(args: &TagArgs) -> Result<(), CliCommandError> {
    match &args.command {
        TagCommand::Search(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for TagSearchArgs {
    type Output = SearchTagsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_tags(&SearchTagsParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_tag_search_result(output)
    }
}

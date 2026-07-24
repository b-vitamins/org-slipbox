use super::super::output::CliCommandError;
use super::super::render::notes::render_content_search_result;
use super::super::render::relations::render_occurrence_search_result;
use super::super::runtime::{HeadlessArgs, HeadlessCommand, run_headless_command};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    SearchNodeContentParams, SearchNodeContentResult, SearchOccurrencesParams,
    SearchOccurrencesResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};

#[derive(Debug, Clone, Args)]
pub(crate) struct SearchArgs {
    #[command(subcommand)]
    pub(crate) command: SearchCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SearchCommand {
    /// Search raw indexed note text occurrences.
    Occurrences(OccurrencesSearchArgs),
    /// Search note body content, ranked, with a highlighted excerpt per hit.
    Content(ContentSearchArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct OccurrencesSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Text query.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum occurrences to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ContentSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Search text matched against indexed note body, title, and aliases.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum hits to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

pub(crate) fn run_search(args: &SearchArgs) -> Result<(), CliCommandError> {
    match &args.command {
        SearchCommand::Occurrences(command) => run_headless_command(command),
        SearchCommand::Content(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for OccurrencesSearchArgs {
    type Output = SearchOccurrencesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_occurrences(&SearchOccurrencesParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_occurrence_search_result(output)
    }
}

impl HeadlessCommand for ContentSearchArgs {
    type Output = SearchNodeContentResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_node_content(&SearchNodeContentParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_content_search_result(output)
    }
}

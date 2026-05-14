use super::output::{CliCommandError, OutputMode, write_output};
use super::render::{
    notes::render_node_summary,
    relations::{
        render_agenda_result, render_occurrence_search_result, render_ref_search_result,
        render_slipbox_link_rewrite_application, render_slipbox_link_rewrite_preview,
        render_tag_search_result,
    },
};
use super::runtime::{
    HeadlessArgs, HeadlessCommand, invalid_request_error, normalize_edit_file_path,
    require_resolved_node, run_headless_command,
};
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use slipbox_core::{
    AgendaParams, AgendaResult, GraphParams, GraphResult, GraphTitleShortening, NodeFromRefParams,
    NodeRecord, SearchOccurrencesParams, SearchOccurrencesResult, SearchRefsParams,
    SearchRefsResult, SearchTagsParams, SearchTagsResult, SlipboxLinkRewriteApplyParams,
    SlipboxLinkRewritePreviewParams, SlipboxLinkRewritePreviewResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct GraphDotFileResult {
    output_path: String,
    format: &'static str,
}

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

#[derive(Debug, Clone, Args)]
pub(crate) struct SearchArgs {
    #[command(subcommand)]
    pub(crate) command: SearchCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SearchCommand {
    /// Search raw indexed note text occurrences.
    Occurrences(OccurrencesSearchArgs),
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
pub(crate) struct AgendaArgs {
    #[command(subcommand)]
    pub(crate) command: AgendaCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum AgendaCommand {
    /// Query entries for the current local date.
    Today(AgendaTodayArgs),
    /// Query entries for one ISO date, YYYY-MM-DD.
    Date(AgendaDateArgs),
    /// Query entries for an inclusive ISO date range, YYYY-MM-DD YYYY-MM-DD.
    Range(AgendaRangeArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaTodayArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaDateArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// ISO date to query, YYYY-MM-DD.
    #[arg(value_name = "DATE")]
    pub(crate) date: String,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AgendaRangeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Inclusive ISO start date, YYYY-MM-DD.
    #[arg(value_name = "START")]
    pub(crate) start: String,
    /// Inclusive ISO end date, YYYY-MM-DD.
    #[arg(value_name = "END")]
    pub(crate) end: String,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct GraphArgs {
    #[command(subcommand)]
    pub(crate) command: GraphCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum GraphCommand {
    /// Emit graph DOT.
    Dot(GraphDotArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum GraphShortenArg {
    Truncate,
    Wrap,
}

impl From<GraphShortenArg> for GraphTitleShortening {
    fn from(value: GraphShortenArg) -> Self {
        match value {
            GraphShortenArg::Truncate => Self::Truncate,
            GraphShortenArg::Wrap => Self::Wrap,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct GraphDotArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Exact root node key for a neighborhood graph.
    #[arg(long, value_name = "KEY")]
    pub(crate) root_node_key: Option<String>,
    /// Maximum graph distance from --root-node-key.
    #[arg(long)]
    pub(crate) max_distance: Option<u32>,
    /// Include disconnected notes in global graph output.
    #[arg(long)]
    pub(crate) include_orphans: bool,
    /// Link type to hide. Only `id` is currently supported.
    #[arg(long = "hide-link-type", value_name = "TYPE")]
    pub(crate) hidden_link_types: Vec<String>,
    /// Maximum graph label length before shortening.
    #[arg(long, default_value_t = 100)]
    pub(crate) max_title_length: usize,
    /// Title shortening mode.
    #[arg(long, value_enum)]
    pub(crate) shorten_titles: Option<GraphShortenArg>,
    /// URL prefix for nodes with explicit IDs.
    #[arg(long, value_name = "PREFIX")]
    pub(crate) node_url_prefix: Option<String>,
    /// Write DOT to this path instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkArgs {
    #[command(subcommand)]
    pub(crate) command: LinkCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum LinkCommand {
    /// Replace exact `slipbox:` Org links with stable `id:` links.
    RewriteSlipbox(LinkRewriteSlipboxArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxArgs {
    #[command(subcommand)]
    pub(crate) command: LinkRewriteSlipboxCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum LinkRewriteSlipboxCommand {
    /// Preview supported `slipbox:` link rewrites in one file.
    Preview(LinkRewriteSlipboxPreviewArgs),
    /// Apply supported `slipbox:` link rewrites in one file after confirmation.
    #[command(
        long_about = "Apply supported `slipbox:` link rewrites in one file. Requires --confirm-replace-slipbox-links and returns changed-file refresh status."
    )]
    Apply(LinkRewriteSlipboxApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to inspect or rewrite, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxPreviewArgs {
    #[command(flatten)]
    pub(crate) target: LinkRewriteSlipboxFileArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxApplyArgs {
    #[command(flatten)]
    pub(crate) target: LinkRewriteSlipboxFileArgs,
    /// Confirm replacing supported `slipbox:` links in the selected file.
    #[arg(long)]
    pub(crate) confirm_replace_slipbox_links: bool,
}

pub(crate) fn run_ref(args: &RefArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RefCommand::Search(command) => run_headless_command(command),
        RefCommand::Resolve(command) => run_headless_command(command),
    }
}

pub(crate) fn run_tag(args: &TagArgs) -> Result<(), CliCommandError> {
    match &args.command {
        TagCommand::Search(command) => run_headless_command(command),
    }
}

pub(crate) fn run_search(args: &SearchArgs) -> Result<(), CliCommandError> {
    match &args.command {
        SearchCommand::Occurrences(command) => run_headless_command(command),
    }
}

pub(crate) fn run_agenda(args: &AgendaArgs) -> Result<(), CliCommandError> {
    match &args.command {
        AgendaCommand::Today(command) => run_headless_command(command),
        AgendaCommand::Date(command) => run_headless_command(command),
        AgendaCommand::Range(command) => run_headless_command(command),
    }
}

pub(crate) fn run_graph(args: &GraphArgs) -> Result<(), CliCommandError> {
    match &args.command {
        GraphCommand::Dot(command) => run_graph_dot(command),
    }
}

pub(crate) fn run_link(args: &LinkArgs) -> Result<(), CliCommandError> {
    match &args.command {
        LinkCommand::RewriteSlipbox(command) => run_link_rewrite_slipbox(command),
    }
}

fn run_graph_dot(command: &GraphDotArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let mut client = command.headless.connect()?;
    let result = client
        .graph_dot(&GraphParams {
            root_node_key: command.root_node_key.clone(),
            max_distance: command.max_distance,
            include_orphans: command.include_orphans,
            hidden_link_types: command.hidden_link_types.clone(),
            max_title_length: command.max_title_length,
            shorten_titles: command.shorten_titles.map(Into::into),
            node_url_prefix: command.node_url_prefix.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    if let Some(output_path) = &command.output
        && output_path != Path::new("-")
    {
        fs::write(output_path, result.dot.as_bytes())
            .with_context(|| format!("failed to write graph DOT to {}", output_path.display()))
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let file_result = GraphDotFileResult {
            output_path: output_path.display().to_string(),
            format: "dot",
        };
        write_output(&mut writer, output_mode, &file_result, |value| {
            format!("wrote graph DOT: {}\n", value.output_path)
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
        return Ok(());
    }

    write_graph_dot_stdout(output_mode, &result)
        .map_err(|error| CliCommandError::new(output_mode, error))
}

fn write_graph_dot_stdout(output_mode: OutputMode, result: &GraphResult) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    match output_mode {
        OutputMode::Human => {
            writer.write_all(result.dot.as_bytes())?;
            if !result.dot.ends_with('\n') {
                writer.write_all(b"\n")?;
            }
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut writer, result)?;
            writer.write_all(b"\n")?;
        }
    }
    writer.flush()?;
    Ok(())
}

impl HeadlessCommand for LinkRewriteSlipboxPreviewArgs {
    type Output = SlipboxLinkRewritePreviewResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.target.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let file_path =
            normalize_edit_file_path(&self.target.headless.scope.root, &self.target.file)?;
        client.slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams { file_path })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_slipbox_link_rewrite_preview(&output.preview)
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

fn parse_agenda_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid_request_error(format!(
            "invalid agenda date {value:?}: expected ISO date YYYY-MM-DD"
        ))
    })
}

fn today_local_date() -> NaiveDate {
    Local::now().date_naive()
}

fn agenda_params(
    start: NaiveDate,
    end: NaiveDate,
    limit: usize,
) -> Result<AgendaParams, DaemonClientError> {
    if end < start {
        return Err(invalid_request_error(format!(
            "agenda range end {} is before start {}",
            end.format("%Y-%m-%d"),
            start.format("%Y-%m-%d")
        )));
    }
    Ok(AgendaParams {
        start: format!("{}T00:00:00", start.format("%Y-%m-%d")),
        end: format!("{}T23:59:59", end.format("%Y-%m-%d")),
        limit,
    })
}

impl HeadlessCommand for AgendaTodayArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let today = today_local_date();
        client.agenda(&agenda_params(today, today, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}

impl HeadlessCommand for AgendaDateArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let date = parse_agenda_date(&self.date)?;
        client.agenda(&agenda_params(date, date, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}

impl HeadlessCommand for AgendaRangeArgs {
    type Output = AgendaResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let start = parse_agenda_date(&self.start)?;
        let end = parse_agenda_date(&self.end)?;
        client.agenda(&agenda_params(start, end, self.limit)?)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_agenda_result(output)
    }
}

fn run_link_rewrite_slipbox(command: &LinkRewriteSlipboxArgs) -> Result<(), CliCommandError> {
    match &command.command {
        LinkRewriteSlipboxCommand::Preview(command) => run_headless_command(command),
        LinkRewriteSlipboxCommand::Apply(command) => run_link_rewrite_slipbox_apply(command),
    }
}

fn run_link_rewrite_slipbox_apply(
    command: &LinkRewriteSlipboxApplyArgs,
) -> Result<(), CliCommandError> {
    let output_mode = command.target.headless.output_mode();
    if !command.confirm_replace_slipbox_links {
        return Err(CliCommandError::new(
            output_mode,
            anyhow::anyhow!("link rewrite apply requires --confirm-replace-slipbox-links"),
        ));
    }

    let mut client = command.target.headless.connect()?;
    let file_path =
        normalize_edit_file_path(&command.target.headless.scope.root, &command.target.file)
            .map_err(|error| CliCommandError::new(output_mode, error))?;
    let preview = client
        .slipbox_link_rewrite_preview(&SlipboxLinkRewritePreviewParams { file_path })
        .map_err(|error| CliCommandError::new(output_mode, error))?
        .preview;
    let output = client
        .slipbox_link_rewrite_apply(&SlipboxLinkRewriteApplyParams {
            expected_preview: preview,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_slipbox_link_rewrite_application(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

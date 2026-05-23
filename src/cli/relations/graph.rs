use super::super::output::{CliCommandError, OutputMode, write_output};
use super::super::runtime::{HeadlessArgs, run_daemon_operation};
use anyhow::{Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use slipbox_core::{GraphParams, GraphResult, GraphTitleShortening};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct GraphDotFileResult {
    output_path: String,
    format: &'static str,
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

pub(crate) fn run_graph(args: &GraphArgs) -> Result<(), CliCommandError> {
    match &args.command {
        GraphCommand::Dot(command) => run_graph_dot(command),
    }
}

fn run_graph_dot(command: &GraphDotArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let result = run_daemon_operation(&command.headless, output_mode, |client| {
        client.graph_dot(&GraphParams {
            root_node_key: command.root_node_key.clone(),
            max_distance: command.max_distance,
            include_orphans: command.include_orphans,
            hidden_link_types: command.hidden_link_types.clone(),
            max_title_length: command.max_title_length,
            shorten_titles: command.shorten_titles.map(Into::into),
            node_url_prefix: command.node_url_prefix.clone(),
        })
    })?;

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

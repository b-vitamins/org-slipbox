use super::super::output::{CliCommandError, write_json_export, write_output};
use super::super::render::explorations::{
    render_artifact_kind, render_artifact_list, render_executed_exploration_artifact,
    render_saved_exploration_artifact,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, run_daemon_operation, run_headless_command,
};
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use slipbox_core::{
    DeleteExplorationArtifactResult, ExecuteExplorationArtifactResult, ExplorationArtifactIdParams,
    ExplorationArtifactResult, ExplorationArtifactSummary, ListExplorationArtifactsResult,
    SaveExplorationArtifactParams, SavedExplorationArtifact,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct ArtifactExportFileResult {
    artifact: ExplorationArtifactSummary,
    output_path: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactArgs {
    #[command(subcommand)]
    pub(crate) command: ArtifactCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ArtifactCommand {
    /// List saved exploration artifacts.
    List(ArtifactListArgs),
    /// Show a saved artifact definition.
    Show(ArtifactShowArgs),
    /// Execute a saved artifact through the live daemon semantics.
    Run(ArtifactRunArgs),
    /// Export a saved artifact definition as stable JSON.
    Export(ArtifactExportArgs),
    /// Import a saved artifact definition from stable JSON.
    Import(ArtifactImportArgs),
    /// Delete a saved artifact by durable identifier.
    Delete(ArtifactDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable saved artifact identifier.
    #[arg(value_name = "ARTIFACT_ID")]
    pub(crate) artifact_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactShowArgs {
    #[command(flatten)]
    pub(crate) artifact: ArtifactIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactRunArgs {
    #[command(flatten)]
    pub(crate) artifact: ArtifactIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactDeleteArgs {
    #[command(flatten)]
    pub(crate) artifact: ArtifactIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactExportArgs {
    #[command(flatten)]
    pub(crate) artifact: ArtifactIdArgs,
    /// Write exported JSON to this path instead of stdout. Use `-` for stdout.
    #[arg(long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactImportArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Read imported JSON from this path, or `-` for stdin.
    #[arg(default_value = "-", value_name = "PATH")]
    pub(crate) input: String,
    /// Replace an existing artifact with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

pub(crate) fn run_artifact(args: &ArtifactArgs) -> Result<(), CliCommandError> {
    match &args.command {
        ArtifactCommand::List(command) => run_headless_command(command),
        ArtifactCommand::Show(command) => run_headless_command(command),
        ArtifactCommand::Run(command) => run_headless_command(command),
        ArtifactCommand::Export(command) => run_artifact_export(command),
        ArtifactCommand::Import(command) => run_artifact_import(command),
        ArtifactCommand::Delete(command) => run_headless_command(command),
    }
}

fn run_artifact_export(command: &ArtifactExportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.artifact.headless.output_mode();
    let artifact = run_daemon_operation(&command.artifact.headless, output_mode, |client| {
        client.exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: command.artifact.artifact_id.clone(),
        })
    })?
    .artifact;

    if write_json_export(
        output_mode,
        command.output.as_deref(),
        &artifact,
        "failed to serialize saved exploration artifact",
        |path| {
            format!(
                "failed to write exported exploration artifact {}",
                path.display()
            )
        },
    )? {
        let output_path = command
            .output
            .as_deref()
            .expect("json export helper only returns true for a concrete output path");
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let result = ArtifactExportFileResult {
            artifact: ExplorationArtifactSummary::from(&artifact),
            output_path: output_path.display().to_string(),
        };
        write_output(&mut writer, output_mode, &result, |value| {
            format!(
                "exported artifact: {} -> {}\n",
                value.artifact.metadata.artifact_id, value.output_path
            )
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
        return Ok(());
    }
    Ok(())
}

fn run_artifact_import(command: &ArtifactImportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let bytes = read_artifact_json_input(&command.input)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let artifact: SavedExplorationArtifact = serde_json::from_slice(&bytes)
        .with_context(|| {
            if command.input == "-" {
                "failed to parse saved exploration artifact JSON from stdin".to_owned()
            } else {
                format!(
                    "failed to parse saved exploration artifact JSON from {}",
                    command.input
                )
            }
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let saved = run_daemon_operation(&command.headless, output_mode, |client| {
        client.save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact,
            overwrite: command.overwrite,
        })
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &saved, |value| {
        format!(
            "imported artifact: {} [{}]\n",
            value.artifact.metadata.artifact_id,
            render_artifact_kind(value.artifact.kind)
        )
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

impl HeadlessCommand for ArtifactListArgs {
    type Output = ListExplorationArtifactsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_exploration_artifacts()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_artifact_list(output)
    }
}

impl HeadlessCommand for ArtifactShowArgs {
    type Output = ExplorationArtifactResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.artifact.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: self.artifact.artifact_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_saved_exploration_artifact(&output.artifact)
    }
}

impl HeadlessCommand for ArtifactRunArgs {
    type Output = ExecuteExplorationArtifactResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.artifact.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.execute_exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: self.artifact.artifact_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_executed_exploration_artifact(&output.artifact)
    }
}

impl HeadlessCommand for ArtifactDeleteArgs {
    type Output = DeleteExplorationArtifactResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.artifact.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: self.artifact.artifact_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted artifact: {}\n", output.artifact_id)
    }
}

fn read_artifact_json_input(input: &str) -> Result<Vec<u8>> {
    if input == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read saved exploration artifact JSON from stdin")?;
        return Ok(bytes);
    }

    fs::read(input)
        .with_context(|| format!("failed to read saved exploration artifact JSON from {input}"))
}

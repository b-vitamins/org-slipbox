use super::runtime::{
    ArtifactExportFileResult, CliCommandError, HeadlessArgs, HeadlessCommand, OutputMode,
    ResolveTarget, ResolveTargetArgs, SaveArtifactArgs, SavedCompareCommandResult,
    SavedExploreCommandResult, resolve_note_target, run_headless_command, write_output,
};
use anyhow::{Context, Result};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use slipbox_core::{
    CompareNotesParams, DeleteExplorationArtifactResult, ExecuteExplorationArtifactResult,
    ExplorationArtifactIdParams, ExplorationArtifactPayload, ExplorationArtifactResult,
    ExplorationArtifactSummary, ExplorationLens, ExploreParams, ExploreResult,
    ListExplorationArtifactsResult, NodeRecord, NoteComparisonGroup, NoteComparisonResult,
    SaveExplorationArtifactParams, SavedComparisonArtifact, SavedExplorationArtifact,
    SavedLensViewArtifact,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::render::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ExploreLensArg {
    Structure,
    Refs,
    Time,
    Tasks,
    Bridges,
    Dormant,
    Unresolved,
}

impl From<ExploreLensArg> for ExplorationLens {
    fn from(value: ExploreLensArg) -> Self {
        match value {
            ExploreLensArg::Structure => Self::Structure,
            ExploreLensArg::Refs => Self::Refs,
            ExploreLensArg::Time => Self::Time,
            ExploreLensArg::Tasks => Self::Tasks,
            ExploreLensArg::Bridges => Self::Bridges,
            ExploreLensArg::Dormant => Self::Dormant,
            ExploreLensArg::Unresolved => Self::Unresolved,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ExploreArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Declared exploration lens to execute.
    #[arg(long, value_enum)]
    pub(crate) lens: ExploreLensArg,
    /// Maximum entries per section.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    /// Deduplicate structural backlinks and forward links by source/destination note.
    #[arg(long)]
    pub(crate) unique: bool,
    #[command(flatten)]
    pub(crate) save: SaveArtifactArgs,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("left-target")
        .args(["left_id", "left_title", "left_reference", "left_key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct CompareLeftTargetArgs {
    /// Resolve the left note by exact explicit Org ID.
    #[arg(long = "left-id", group = "left-target")]
    pub(crate) left_id: Option<String>,
    /// Resolve the left note by exact title or alias.
    #[arg(long = "left-title", group = "left-target")]
    pub(crate) left_title: Option<String>,
    /// Resolve the left note by exact reference.
    #[arg(long = "left-ref", group = "left-target")]
    pub(crate) left_reference: Option<String>,
    /// Resolve the left note by exact node key.
    #[arg(long = "left-key", group = "left-target")]
    pub(crate) left_key: Option<String>,
}

impl CompareLeftTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.left_id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.left_title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.left_reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.left_key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one left target selector");
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("right-target")
        .args(["right_id", "right_title", "right_reference", "right_key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct CompareRightTargetArgs {
    /// Resolve the right note by exact explicit Org ID.
    #[arg(long = "right-id", group = "right-target")]
    pub(crate) right_id: Option<String>,
    /// Resolve the right note by exact title or alias.
    #[arg(long = "right-title", group = "right-target")]
    pub(crate) right_title: Option<String>,
    /// Resolve the right note by exact reference.
    #[arg(long = "right-ref", group = "right-target")]
    pub(crate) right_reference: Option<String>,
    /// Resolve the right note by exact node key.
    #[arg(long = "right-key", group = "right-target")]
    pub(crate) right_key: Option<String>,
}

impl CompareRightTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.right_id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.right_title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.right_reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.right_key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one right target selector");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CompareGroupArg {
    All,
    Overlap,
    Divergence,
    Tension,
}

impl From<CompareGroupArg> for NoteComparisonGroup {
    fn from(value: CompareGroupArg) -> Self {
        match value {
            CompareGroupArg::All => Self::All,
            CompareGroupArg::Overlap => Self::Overlap,
            CompareGroupArg::Divergence => Self::Divergence,
            CompareGroupArg::Tension => Self::Tension,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CompareArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) left: CompareLeftTargetArgs,
    #[command(flatten)]
    pub(crate) right: CompareRightTargetArgs,
    /// Comparison group to retain in the output.
    #[arg(long, value_enum, default_value_t = CompareGroupArg::All)]
    pub(crate) group: CompareGroupArg,
    /// Maximum entries per comparison section.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) save: SaveArtifactArgs,
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
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ArtifactImportArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Read imported JSON from this path, or `-` for stdin.
    #[arg(default_value = "-")]
    pub(crate) input: String,
    /// Replace an existing artifact with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

fn execute_live_explore(
    command: &ExploreArgs,
    client: &mut DaemonClient,
) -> Result<(String, ExploreResult), DaemonClientError> {
    let focus_node_key = resolve_explore_focus_node_key(client, &command.target.target())?;
    let result = client.explore(&ExploreParams {
        node_key: focus_node_key.clone(),
        lens: command.lens.into(),
        limit: command.limit,
        unique: command.unique,
    })?;
    Ok((focus_node_key, result))
}

fn execute_live_compare(
    command: &CompareArgs,
    client: &mut DaemonClient,
) -> Result<(NodeRecord, NodeRecord, NoteComparisonResult), DaemonClientError> {
    let left = resolve_note_target(client, &command.left.target())?;
    let right = resolve_note_target(client, &command.right.target())?;
    let result = client
        .compare_notes(&CompareNotesParams {
            left_node_key: left.node_key.clone(),
            right_node_key: right.node_key.clone(),
            limit: command.limit,
        })?
        .filtered_to_group(command.group.into());
    Ok((left, right, result))
}

fn resolve_explore_focus_node_key(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<String, DaemonClientError> {
    match target {
        ResolveTarget::Key(node_key) => Ok(node_key.clone()),
        _ => resolve_note_target(client, target).map(|node| node.node_key),
    }
}

pub(crate) fn run_explore(args: &ExploreArgs) -> Result<(), CliCommandError> {
    let output_mode = args.headless.output_mode();
    let Some(save_request) = args
        .save
        .request()
        .map_err(|error| CliCommandError::new(output_mode, error))?
    else {
        return run_headless_command(args);
    };

    let mut client = args.headless.connect()?;
    let (focus_node_key, result) = execute_live_explore(args, &mut client)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let artifact = SavedExplorationArtifact {
        metadata: save_request.metadata,
        payload: ExplorationArtifactPayload::LensView {
            artifact: Box::new(SavedLensViewArtifact {
                root_node_key: focus_node_key.clone(),
                current_node_key: focus_node_key,
                lens: args.lens.into(),
                limit: args.limit,
                unique: args.unique,
                frozen_context: false,
            }),
        },
    };
    let saved = client
        .save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact,
            overwrite: save_request.overwrite,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let command_result = SavedExploreCommandResult {
        result,
        artifact: saved.artifact,
    };
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &command_result, |value| {
        let mut output = render_explore_result(&value.result);
        output.push('\n');
        output.push_str(&render_saved_artifact_summary(&value.artifact));
        output
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

pub(crate) fn run_compare(args: &CompareArgs) -> Result<(), CliCommandError> {
    let output_mode = args.headless.output_mode();
    let Some(save_request) = args
        .save
        .request()
        .map_err(|error| CliCommandError::new(output_mode, error))?
    else {
        return run_headless_command(args);
    };

    let mut client = args.headless.connect()?;
    let (left, right, result) = execute_live_compare(args, &mut client)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let artifact = SavedExplorationArtifact {
        metadata: save_request.metadata,
        payload: ExplorationArtifactPayload::Comparison {
            artifact: Box::new(SavedComparisonArtifact {
                root_node_key: left.node_key.clone(),
                left_node_key: left.node_key,
                right_node_key: right.node_key,
                active_lens: ExplorationLens::Structure,
                structure_unique: false,
                comparison_group: args.group.into(),
                limit: args.limit,
                frozen_context: false,
            }),
        },
    };
    let saved = client
        .save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact,
            overwrite: save_request.overwrite,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let command_result = SavedCompareCommandResult {
        result,
        artifact: saved.artifact,
    };
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &command_result, |value| {
        let mut output = render_compare_result(&value.result, args.group.into());
        output.push('\n');
        output.push_str(&render_saved_artifact_summary(&value.artifact));
        output
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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
    let mut client = command.artifact.headless.connect()?;
    let artifact = client
        .exploration_artifact(&ExplorationArtifactIdParams {
            artifact_id: command.artifact.artifact_id.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?
        .artifact;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    if let Some(output_path) = &command.output
        && output_path != Path::new("-")
    {
        let serialized = serde_json::to_vec_pretty(&artifact)
            .context("failed to serialize saved exploration artifact")
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        fs::write(output_path, serialized)
            .with_context(|| {
                format!(
                    "failed to write exported exploration artifact {}",
                    output_path.display()
                )
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;

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

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    match output_mode {
        OutputMode::Human => {
            serde_json::to_writer_pretty(&mut writer, &artifact)
                .context("failed to serialize saved exploration artifact")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
            writer
                .write_all(b"\n")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut writer, &artifact)
                .context("failed to serialize saved exploration artifact")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
            writer
                .write_all(b"\n")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
        }
    }
    writer
        .flush()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
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

    let mut client = command.headless.connect()?;
    let saved = client
        .save_exploration_artifact(&slipbox_core::SaveExplorationArtifactParams {
            artifact,
            overwrite: command.overwrite,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

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

impl HeadlessCommand for ExploreArgs {
    type Output = ExploreResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        execute_live_explore(self, client).map(|(_, result)| result)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_explore_result(output)
    }
}

impl HeadlessCommand for CompareArgs {
    type Output = NoteComparisonResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        execute_live_compare(self, client).map(|(_, _, result)| result)
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_compare_result(output, self.group.into())
    }
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

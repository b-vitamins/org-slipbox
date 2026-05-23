use super::super::output::{CliCommandError, write_output};
use super::super::render::explorations::{render_explore_result, render_saved_artifact_summary};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTarget, ResolveTargetArgs, SaveArtifactArgs,
    resolve_note_target, run_daemon_operation, run_headless_command,
};
use anyhow::Result;
use clap::{Args, ValueEnum};
use serde::Serialize;
use slipbox_core::{
    ExplorationArtifactSummary, ExplorationLens, ExploreParams, ExploreResult,
    SaveExplorationArtifactParams, SavedExplorationArtifact,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;

#[derive(Debug, Serialize)]
struct SavedExploreCommandResult {
    result: ExploreResult,
    artifact: ExplorationArtifactSummary,
}

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
    #[arg(long, value_enum, value_name = "LENS")]
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

    let (result, saved) = run_daemon_operation(&args.headless, output_mode, |client| {
        let (focus_node_key, result) = execute_live_explore(args, client)?;
        let artifact = SavedExplorationArtifact::live_lens_view(
            save_request.metadata,
            focus_node_key,
            args.lens.into(),
            args.limit,
            args.unique,
        );
        let saved = client.save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact,
            overwrite: save_request.overwrite,
        })?;
        Ok((result, saved))
    })?;

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

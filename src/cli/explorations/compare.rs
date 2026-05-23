use super::super::output::{CliCommandError, write_output};
use super::super::render::explorations::{render_compare_result, render_saved_artifact_summary};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTarget, SaveArtifactArgs, resolve_note_target,
    run_daemon_operation, run_headless_command,
};
use anyhow::Result;
use clap::{ArgGroup, Args, ValueEnum};
use serde::Serialize;
use slipbox_core::{
    CompareNotesParams, ExplorationArtifactSummary, NodeRecord, NoteComparisonGroup,
    NoteComparisonResult, SaveExplorationArtifactParams, SavedExplorationArtifact,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;

#[derive(Debug, Serialize)]
struct SavedCompareCommandResult {
    result: NoteComparisonResult,
    artifact: ExplorationArtifactSummary,
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
    #[arg(long = "left-id", group = "left-target", value_name = "ID")]
    pub(crate) left_id: Option<String>,
    /// Resolve the left note by exact title or alias.
    #[arg(long = "left-title", group = "left-target", value_name = "TITLE")]
    pub(crate) left_title: Option<String>,
    /// Resolve the left note by exact reference.
    #[arg(long = "left-ref", group = "left-target", value_name = "REF")]
    pub(crate) left_reference: Option<String>,
    /// Resolve the left note by exact node key.
    #[arg(long = "left-key", group = "left-target", value_name = "KEY")]
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
    #[arg(long = "right-id", group = "right-target", value_name = "ID")]
    pub(crate) right_id: Option<String>,
    /// Resolve the right note by exact title or alias.
    #[arg(long = "right-title", group = "right-target", value_name = "TITLE")]
    pub(crate) right_title: Option<String>,
    /// Resolve the right note by exact reference.
    #[arg(long = "right-ref", group = "right-target", value_name = "REF")]
    pub(crate) right_reference: Option<String>,
    /// Resolve the right note by exact node key.
    #[arg(long = "right-key", group = "right-target", value_name = "KEY")]
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
    #[arg(long, value_enum, default_value_t = CompareGroupArg::All, value_name = "GROUP")]
    pub(crate) group: CompareGroupArg,
    /// Maximum entries per comparison section.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) save: SaveArtifactArgs,
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

pub(crate) fn run_compare(args: &CompareArgs) -> Result<(), CliCommandError> {
    let output_mode = args.headless.output_mode();
    let Some(save_request) = args
        .save
        .request()
        .map_err(|error| CliCommandError::new(output_mode, error))?
    else {
        return run_headless_command(args);
    };

    let (result, saved) = run_daemon_operation(&args.headless, output_mode, |client| {
        let (left, right, result) = execute_live_compare(args, client)?;
        let artifact = SavedExplorationArtifact::live_comparison(
            save_request.metadata,
            left.node_key,
            right.node_key,
            args.group.into(),
            args.limit,
        );
        let saved = client.save_exploration_artifact(&SaveExplorationArtifactParams {
            artifact,
            overwrite: save_request.overwrite,
        })?;
        Ok((result, saved))
    })?;

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

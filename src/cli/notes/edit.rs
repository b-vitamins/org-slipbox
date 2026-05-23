use super::super::output::CliCommandError;
use super::super::render::notes::render_structural_write_report;
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTarget, invalid_request_error, normalize_edit_file_path,
    resolve_anchor_or_note_target_key, resolve_note_target, run_headless_command,
    validate_region_range,
};
use anyhow::Result;
use clap::{ArgGroup, Args, Subcommand};
use slipbox_core::{
    ExtractSubtreeParams, RefileRegionParams, RefileSubtreeParams, RewriteFileParams,
    StructuralWriteReport,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub(crate) struct EditArgs {
    #[command(subcommand)]
    pub(crate) command: EditCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum EditCommand {
    /// Move an indexed subtree under an exact target note.
    RefileSubtree(EditRefileSubtreeArgs),
    /// Move a character range under an exact target note.
    RefileRegion(EditRefileRegionArgs),
    /// Extract an indexed subtree into a file note.
    ExtractSubtree(EditExtractSubtreeArgs),
    /// Promote a single root heading into file-level metadata.
    PromoteFile(EditPromoteFileArgs),
    /// Demote file-level metadata into a single root heading.
    DemoteFile(EditDemoteFileArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditRefileSubtreeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) source: EditSourceTargetArgs,
    #[command(flatten)]
    pub(crate) target: EditTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditRefileRegionArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Source file path, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// 1-based start character position.
    #[arg(long)]
    pub(crate) start: u32,
    /// 1-based end character position.
    #[arg(long)]
    pub(crate) end: u32,
    #[command(flatten)]
    pub(crate) target: EditTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditExtractSubtreeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) source: EditSourceTargetArgs,
    /// Destination file path, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditPromoteFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to rewrite, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditDemoteFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to rewrite, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("source-target")
        .args(["source_id", "source_title", "source_reference", "source_key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct EditSourceTargetArgs {
    /// Resolve the source anchor by exact explicit Org ID.
    #[arg(long = "source-id", group = "source-target", value_name = "ID")]
    pub(crate) source_id: Option<String>,
    /// Resolve the source anchor by exact title or alias.
    #[arg(long = "source-title", group = "source-target", value_name = "TITLE")]
    pub(crate) source_title: Option<String>,
    /// Resolve the source anchor by exact reference.
    #[arg(long = "source-ref", group = "source-target", value_name = "REF")]
    pub(crate) source_reference: Option<String>,
    /// Use an exact source node key. This may be an anonymous heading anchor.
    #[arg(long = "source-key", group = "source-target", value_name = "KEY")]
    pub(crate) source_key: Option<String>,
}

impl EditSourceTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.source_id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.source_title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.source_reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.source_key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one source target selector");
        }
    }
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("edit-target")
        .args(["target_id", "target_title", "target_reference", "target_key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct EditTargetArgs {
    /// Resolve the target note by exact explicit Org ID.
    #[arg(long = "target-id", group = "edit-target", value_name = "ID")]
    pub(crate) target_id: Option<String>,
    /// Resolve the target note by exact title or alias.
    #[arg(long = "target-title", group = "edit-target", value_name = "TITLE")]
    pub(crate) target_title: Option<String>,
    /// Resolve the target note by exact reference.
    #[arg(long = "target-ref", group = "edit-target", value_name = "REF")]
    pub(crate) target_reference: Option<String>,
    /// Resolve the target note by exact node key.
    #[arg(long = "target-key", group = "edit-target", value_name = "KEY")]
    pub(crate) target_key: Option<String>,
}

impl EditTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.target_id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.target_title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.target_reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.target_key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one edit target selector");
        }
    }
}

pub(crate) fn run_edit(args: &EditArgs) -> Result<(), CliCommandError> {
    match &args.command {
        EditCommand::RefileSubtree(command) => run_headless_command(command),
        EditCommand::RefileRegion(command) => run_headless_command(command),
        EditCommand::ExtractSubtree(command) => run_headless_command(command),
        EditCommand::PromoteFile(command) => run_headless_command(command),
        EditCommand::DemoteFile(command) => run_headless_command(command),
    }
}

impl HeadlessCommand for EditRefileSubtreeArgs {
    type Output = StructuralWriteReport;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let source_node_key = resolve_anchor_or_note_target_key(client, &self.source.target())?;
        let target = resolve_note_target(client, &self.target.target())?;
        if source_node_key == target.node_key {
            return Err(invalid_request_error(
                "source and target nodes must be different",
            ));
        }
        client.refile_subtree(&RefileSubtreeParams {
            source_node_key,
            target_node_key: target.node_key,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_structural_write_report(output)
    }
}

impl HeadlessCommand for EditRefileRegionArgs {
    type Output = StructuralWriteReport;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        validate_region_range(self.start, self.end)?;
        let file_path = normalize_edit_file_path(&self.headless.scope.root, &self.file)?;
        let target = resolve_note_target(client, &self.target.target())?;
        client.refile_region(&RefileRegionParams {
            file_path,
            start: self.start,
            end: self.end,
            target_node_key: target.node_key,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_structural_write_report(output)
    }
}

impl HeadlessCommand for EditExtractSubtreeArgs {
    type Output = StructuralWriteReport;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let source_node_key = resolve_anchor_or_note_target_key(client, &self.source.target())?;
        let file_path = normalize_edit_file_path(&self.headless.scope.root, &self.file)?;
        client.extract_subtree(&ExtractSubtreeParams {
            source_node_key,
            file_path,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_structural_write_report(output)
    }
}

impl HeadlessCommand for EditPromoteFileArgs {
    type Output = StructuralWriteReport;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let file_path = normalize_edit_file_path(&self.headless.scope.root, &self.file)?;
        client.promote_entire_file(&RewriteFileParams { file_path })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_structural_write_report(output)
    }
}

impl HeadlessCommand for EditDemoteFileArgs {
    type Output = StructuralWriteReport;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let file_path = normalize_edit_file_path(&self.headless.scope.root, &self.file)?;
        client.demote_entire_file(&RewriteFileParams { file_path })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_structural_write_report(output)
    }
}

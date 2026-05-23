use super::super::output::{CliCommandError, write_output};
use super::super::render::notes::{
    render_anchor_summary, render_backlinks_result, render_forward_links_result,
    render_node_search_result, render_node_summary, render_random_node_result,
};
use super::super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTargetArgs, require_resolved_anchor,
    resolve_anchor_or_note_target_key, resolve_note_target, run_daemon_operation,
    run_headless_command,
};
use anyhow::Result;
use clap::{Args, Subcommand};
use slipbox_core::{
    AnchorRecord, BacklinksParams, BacklinksResult, EnsureNodeIdParams, ForwardLinksParams,
    ForwardLinksResult, NodeAtPointParams, NodeRecord, RandomNodeResult, SearchNodesParams,
    SearchNodesResult, UpdateNodeMetadataParams,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeArgs {
    #[command(subcommand)]
    pub(crate) command: NodeCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum NodeCommand {
    /// Show one exact note target.
    Show(NodeShowArgs),
    /// Search indexed notes and headings.
    Search(NodeSearchArgs),
    /// Return one random indexed note.
    Random(NodeRandomArgs),
    /// Show notes linking to one exact note target.
    Backlinks(NodeBacklinksArgs),
    /// Show notes linked from one exact note target.
    ForwardLinks(NodeForwardLinksArgs),
    /// Resolve the indexed anchor at a file line.
    AtPoint(NodeAtPointArgs),
    /// Ensure one indexed anchor has an explicit Org ID.
    EnsureId(NodeEnsureIdArgs),
    /// Show metadata for one exact note target.
    Metadata(NodeMetadataArgs),
    /// Update aliases for one exact note target.
    Alias(NodeMetadataFieldArgs),
    /// Update references for one exact note target.
    Ref(NodeMetadataFieldArgs),
    /// Update tags for one exact note target.
    Tag(NodeMetadataFieldArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Search text matched against indexed note titles, aliases, refs, and body text.
    #[arg(value_name = "QUERY")]
    pub(crate) query: String,
    /// Maximum nodes to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeRandomArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeBacklinksArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Maximum backlinks to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    /// Deduplicate backlinks by source note.
    #[arg(long)]
    pub(crate) unique: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeForwardLinksArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Maximum forward links to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    /// Deduplicate forward links by destination note.
    #[arg(long)]
    pub(crate) unique: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeAtPointArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to inspect, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// 1-based line number.
    #[arg(long)]
    pub(crate) line: u32,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeEnsureIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataArgs {
    #[command(subcommand)]
    pub(crate) command: NodeMetadataCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum NodeMetadataCommand {
    /// Show aliases, references, and tags for one exact note target.
    Show(NodeMetadataShowArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataFieldArgs {
    #[command(subcommand)]
    pub(crate) command: NodeMetadataFieldCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum NodeMetadataFieldCommand {
    /// Add values while preserving existing metadata.
    Add(NodeMetadataValuesArgs),
    /// Remove values while preserving other metadata.
    Remove(NodeMetadataValuesArgs),
    /// Replace the full metadata list. Omit values to clear it.
    Set(NodeMetadataSetArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataValuesArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Metadata values to add or remove.
    #[arg(required = true, value_name = "VALUE")]
    pub(crate) values: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataSetArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Complete metadata values to keep. Omit all values to clear the list.
    #[arg(value_name = "VALUE")]
    pub(crate) values: Vec<String>,
}

pub(crate) fn run_node(args: &NodeArgs) -> Result<(), CliCommandError> {
    match &args.command {
        NodeCommand::Show(command) => run_headless_command(command),
        NodeCommand::Search(command) => run_headless_command(command),
        NodeCommand::Random(command) => run_headless_command(command),
        NodeCommand::Backlinks(command) => run_headless_command(command),
        NodeCommand::ForwardLinks(command) => run_headless_command(command),
        NodeCommand::AtPoint(command) => run_headless_command(command),
        NodeCommand::EnsureId(command) => run_headless_command(command),
        NodeCommand::Metadata(command) => run_node_metadata(command),
        NodeCommand::Alias(command) => run_node_metadata_field(command, MetadataField::Aliases),
        NodeCommand::Ref(command) => run_node_metadata_field(command, MetadataField::Refs),
        NodeCommand::Tag(command) => run_node_metadata_field(command, MetadataField::Tags),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataField {
    Aliases,
    Refs,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataAction {
    Add,
    Remove,
    Set,
}

fn metadata_values_for_action(
    node: &NodeRecord,
    field: MetadataField,
    action: MetadataAction,
    values: &[String],
) -> Vec<String> {
    match action {
        MetadataAction::Add => {
            let mut updated = current_metadata_values(node, field);
            updated.extend(values.iter().cloned());
            updated
        }
        MetadataAction::Remove => {
            let removals = normalized_metadata_values(field, values.to_vec());
            current_metadata_values(node, field)
                .into_iter()
                .filter(|value| {
                    !removals
                        .iter()
                        .any(|removal| removal.eq_ignore_ascii_case(value))
                })
                .collect()
        }
        MetadataAction::Set => values.to_vec(),
    }
}

fn current_metadata_values(node: &NodeRecord, field: MetadataField) -> Vec<String> {
    match field {
        MetadataField::Aliases => node.aliases.clone(),
        MetadataField::Refs => node.refs.clone(),
        MetadataField::Tags => node.tags.clone(),
    }
}

fn normalized_metadata_values(field: MetadataField, values: Vec<String>) -> Vec<String> {
    let params = metadata_update_params(String::new(), field, values);
    match field {
        MetadataField::Aliases => params.normalized_aliases(),
        MetadataField::Refs => params.normalized_refs(),
        MetadataField::Tags => params.normalized_tags(),
    }
    .unwrap_or_default()
}

fn metadata_update_params(
    node_key: String,
    field: MetadataField,
    values: Vec<String>,
) -> UpdateNodeMetadataParams {
    match field {
        MetadataField::Aliases => UpdateNodeMetadataParams {
            node_key,
            aliases: Some(values),
            refs: None,
            tags: None,
        },
        MetadataField::Refs => UpdateNodeMetadataParams {
            node_key,
            aliases: None,
            refs: Some(values),
            tags: None,
        },
        MetadataField::Tags => UpdateNodeMetadataParams {
            node_key,
            aliases: None,
            refs: None,
            tags: Some(values),
        },
    }
}

fn run_node_metadata(args: &NodeMetadataArgs) -> Result<(), CliCommandError> {
    match &args.command {
        NodeMetadataCommand::Show(command) => run_headless_command(command),
    }
}

fn run_node_metadata_field(
    args: &NodeMetadataFieldArgs,
    field: MetadataField,
) -> Result<(), CliCommandError> {
    match &args.command {
        NodeMetadataFieldCommand::Add(command) => run_node_metadata_update(
            &command.headless,
            &command.target,
            field,
            MetadataAction::Add,
            &command.values,
        ),
        NodeMetadataFieldCommand::Remove(command) => run_node_metadata_update(
            &command.headless,
            &command.target,
            field,
            MetadataAction::Remove,
            &command.values,
        ),
        NodeMetadataFieldCommand::Set(command) => run_node_metadata_update(
            &command.headless,
            &command.target,
            field,
            MetadataAction::Set,
            &command.values,
        ),
    }
}

fn run_node_metadata_update(
    headless: &HeadlessArgs,
    target: &ResolveTargetArgs,
    field: MetadataField,
    action: MetadataAction,
    values: &[String],
) -> Result<(), CliCommandError> {
    let output_mode = headless.output_mode();
    let updated = run_daemon_operation(headless, output_mode, |client| {
        let node = resolve_note_target(client, &target.target())?;
        let updated_values = metadata_values_for_action(&node, field, action, values);
        client.update_node_metadata(&metadata_update_params(
            node.node_key.clone(),
            field,
            updated_values,
        ))
    })?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &updated, render_node_summary)
        .map_err(|error| CliCommandError::new(output_mode, error))
}

impl HeadlessCommand for NodeShowArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        resolve_note_target(client, &self.target.target())
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NodeSearchArgs {
    type Output = SearchNodesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_nodes(&SearchNodesParams {
            query: self.query.clone(),
            limit: self.limit,
            sort: None,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_search_result(output)
    }
}

impl HeadlessCommand for NodeRandomArgs {
    type Output = RandomNodeResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.random_node()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_random_node_result(output)
    }
}

impl HeadlessCommand for NodeBacklinksArgs {
    type Output = BacklinksResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.backlinks(&BacklinksParams {
            node_key: node.node_key,
            limit: self.limit,
            unique: self.unique,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_backlinks_result(output)
    }
}

impl HeadlessCommand for NodeForwardLinksArgs {
    type Output = ForwardLinksResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.forward_links(&ForwardLinksParams {
            node_key: node.node_key,
            limit: self.limit,
            unique: self.unique,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_forward_links_result(output)
    }
}

impl HeadlessCommand for NodeAtPointArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        require_resolved_anchor(
            client.anchor_at_point(&NodeAtPointParams {
                file_path: self.file.display().to_string(),
                line: self.line,
            })?,
            format!(
                "no indexed anchor at {}:{}",
                self.file.display(),
                self.line.max(1)
            ),
        )
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NodeEnsureIdArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node_key = resolve_anchor_or_note_target_key(client, &self.target.target())?;
        client.ensure_node_id(&EnsureNodeIdParams { node_key })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NodeMetadataShowArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        resolve_note_target(client, &self.target.target())
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

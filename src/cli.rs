use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{ArgGroup, Args, ValueEnum};
use serde::{Deserialize, Serialize};
use slipbox_core::{
    AnchorRecord, CompareNotesParams, ComparisonConnectorDirection, ExplorationEntry,
    ExplorationExplanation, ExplorationLens, ExplorationSectionKind, ExploreParams, ExploreResult,
    NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord,
    NoteComparisonEntry, NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult,
    NoteComparisonSectionKind, PlanningField, PlanningRelationRecord, StatusInfo,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError, DaemonServeConfig};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::JsonRpcErrorObject;

#[derive(Debug, Clone, Args)]
pub(crate) struct ScopeArgs {
    /// Root directory containing Org files.
    #[arg(long)]
    pub(crate) root: PathBuf,
    /// SQLite database path.
    #[arg(long)]
    pub(crate) db: PathBuf,
    /// File extensions eligible for discovery and indexing.
    #[arg(long = "file-extension")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expressions to exclude from discovery.
    #[arg(long = "exclude-regexp")]
    pub(crate) exclude_regexps: Vec<String>,
}

impl ScopeArgs {
    pub(crate) fn discovery_policy(&self) -> Result<DiscoveryPolicy> {
        if self.file_extensions.is_empty() && self.exclude_regexps.is_empty() {
            Ok(DiscoveryPolicy::default())
        } else {
            DiscoveryPolicy::new(self.file_extensions.clone(), self.exclude_regexps.clone())
        }
    }

    #[must_use]
    pub(crate) fn daemon_config(&self) -> DaemonServeConfig {
        DaemonServeConfig {
            root: self.root.clone(),
            db: self.db.clone(),
            file_extensions: self.file_extensions.clone(),
            exclude_regexps: self.exclude_regexps.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputMode {
    Human,
    Json,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct HeadlessArgs {
    #[command(flatten)]
    pub(crate) scope: ScopeArgs,
    /// Path to the slipbox executable used to spawn `slipbox serve`.
    #[arg(long)]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit structured JSON to stdout and structured errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
}

impl HeadlessArgs {
    #[must_use]
    pub(crate) fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }

    pub(crate) fn server_program_path(&self) -> Result<PathBuf> {
        match &self.server_program {
            Some(path) => Ok(path.clone()),
            None => env::current_exe().context("failed to resolve current slipbox executable"),
        }
    }

    pub(crate) fn connect(&self) -> Result<DaemonClient, CliCommandError> {
        let program = self
            .server_program_path()
            .map_err(|error| CliCommandError::new(self.output_mode(), error))?;
        DaemonClient::spawn(program, &self.scope.daemon_config())
            .map_err(|error| CliCommandError::new(self.output_mode(), error))
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct StatusArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("target")
        .args(["id", "title", "reference", "key"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct ResolveTargetArgs {
    /// Resolve an exact explicit Org ID.
    #[arg(long, group = "target")]
    pub(crate) id: Option<String>,
    /// Resolve an exact title or alias.
    #[arg(long, group = "target")]
    pub(crate) title: Option<String>,
    /// Resolve an exact reference.
    #[arg(long = "ref", group = "target")]
    pub(crate) reference: Option<String>,
    /// Resolve an exact node key.
    #[arg(long, group = "target")]
    pub(crate) key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveTarget {
    Id(String),
    Title(String),
    Reference(String),
    Key(String),
}

impl ResolveTargetArgs {
    #[must_use]
    pub(crate) fn target(&self) -> ResolveTarget {
        if let Some(id) = &self.id {
            ResolveTarget::Id(id.clone())
        } else if let Some(title) = &self.title {
            ResolveTarget::Title(title.clone())
        } else if let Some(reference) = &self.reference {
            ResolveTarget::Reference(reference.clone())
        } else if let Some(node_key) = &self.key {
            ResolveTarget::Key(node_key.clone())
        } else {
            unreachable!("clap enforces exactly one target selector");
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ResolveNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
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
    #[arg(long, value_enum)]
    pub(crate) lens: ExploreLensArg,
    /// Maximum entries per section.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    /// Deduplicate structural backlinks and forward links by source/destination note.
    #[arg(long)]
    pub(crate) unique: bool,
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
}

pub(crate) trait HeadlessCommand {
    type Output: Serialize;

    fn headless_args(&self) -> &HeadlessArgs;
    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError>;
    fn render_human(&self, output: &Self::Output) -> String;
}

pub(crate) fn run_status(args: &StatusArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn run_resolve_node(args: &ResolveNodeArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn run_explore(args: &ExploreArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn run_compare(args: &CompareArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
}

pub(crate) fn report_error(error: &CliCommandError) -> ExitCode {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = error.write(&mut writer);
    ExitCode::from(1)
}

pub(crate) struct CliCommandError {
    output_mode: OutputMode,
    inner: anyhow::Error,
}

impl CliCommandError {
    pub(crate) fn new(output_mode: OutputMode, error: impl Into<anyhow::Error>) -> Self {
        Self {
            output_mode,
            inner: error.into(),
        }
    }

    fn write(&self, writer: &mut impl Write) -> Result<()> {
        match self.output_mode {
            OutputMode::Human => {
                writeln!(writer, "error: {}", self.inner)?;
            }
            OutputMode::Json => {
                let payload = ErrorPayload {
                    error: ErrorMessage {
                        message: self.inner.to_string(),
                    },
                };
                serde_json::to_writer(&mut *writer, &payload)?;
                writer.write_all(b"\n")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn run_headless_command<C>(command: &C) -> Result<(), CliCommandError>
where
    C: HeadlessCommand,
{
    let output_mode = command.headless_args().output_mode();
    let mut client = command.headless_args().connect()?;
    let output = command
        .execute(&mut client)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        command.render_human(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn write_output<T>(
    writer: &mut impl Write,
    output_mode: OutputMode,
    value: &T,
    human_renderer: impl FnOnce(&T) -> String,
) -> Result<()>
where
    T: Serialize,
{
    match output_mode {
        OutputMode::Human => {
            writer.write_all(human_renderer(value).as_bytes())?;
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut *writer, value)?;
            writer.write_all(b"\n")?;
        }
    }
    writer.flush()?;
    Ok(())
}

impl HeadlessCommand for StatusArgs {
    type Output = StatusInfo;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.status()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!(
            "version: {}\nroot: {}\ndb: {}\nfiles indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
            output.version,
            output.root,
            output.db,
            output.files_indexed,
            output.nodes_indexed,
            output.links_indexed,
        )
    }
}

impl HeadlessCommand for ResolveNodeArgs {
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

impl HeadlessCommand for ExploreArgs {
    type Output = ExploreResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let focus_node_key = resolve_explore_focus_node_key(client, &self.target.target())?;
        client.explore(&ExploreParams {
            node_key: focus_node_key,
            lens: self.lens.into(),
            limit: self.limit,
            unique: self.unique,
        })
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
        let left = resolve_note_target(client, &self.left.target())?;
        let right = resolve_note_target(client, &self.right.target())?;
        let result = client.compare_notes(&CompareNotesParams {
            left_node_key: left.node_key,
            right_node_key: right.node_key,
            limit: self.limit,
        })?;
        Ok(result.filtered_to_group(self.group.into()))
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_compare_result(output, self.group.into())
    }
}

pub(crate) fn resolve_note_target(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<NodeRecord, DaemonClientError> {
    match target {
        ResolveTarget::Id(id) => require_resolved_node(
            client.node_from_id(&NodeFromIdParams { id: id.clone() })?,
            format!("unknown node id: {id}"),
        ),
        ResolveTarget::Title(title_or_alias) => require_resolved_node(
            client.node_from_title_or_alias(&NodeFromTitleOrAliasParams {
                title_or_alias: title_or_alias.clone(),
                nocase: false,
            })?,
            format!("unknown node title or alias: {title_or_alias}"),
        ),
        ResolveTarget::Reference(reference) => require_resolved_node(
            client.node_from_ref(&NodeFromRefParams {
                reference: reference.clone(),
            })?,
            format!("unknown node ref: {reference}"),
        ),
        ResolveTarget::Key(node_key) => require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: node_key.clone(),
            })?,
            format!("unknown node key: {node_key}"),
        ),
    }
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

fn require_resolved_node(
    node: Option<NodeRecord>,
    error_message: String,
) -> Result<NodeRecord, DaemonClientError> {
    node.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

fn render_node_summary(node: &NodeRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("node key: {}\n", node.node_key));
    if let Some(explicit_id) = &node.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", node.title));
    output.push_str(&format!("kind: {}\n", node.kind.as_str()));
    output.push_str(&format!("file: {}\n", node.file_path));
    output.push_str(&format!("line: {}\n", node.line));
    if !node.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", node.outline_path));
    }
    if !node.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", node.aliases.join(", ")));
    }
    if !node.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", node.refs.join(", ")));
    }
    if !node.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", node.tags.join(", ")));
    }
    if let Some(todo_keyword) = &node.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &node.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &node.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &node.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

fn render_explore_result(result: &ExploreResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("lens: {}\n", render_exploration_lens(result.lens)));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_exploration_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_exploration_entry(&mut output, entry);
        }
    }
    output
}

fn render_exploration_lens(lens: ExplorationLens) -> &'static str {
    match lens {
        ExplorationLens::Structure => "structure",
        ExplorationLens::Refs => "refs",
        ExplorationLens::Time => "time",
        ExplorationLens::Tasks => "tasks",
        ExplorationLens::Bridges => "bridges",
        ExplorationLens::Dormant => "dormant",
        ExplorationLens::Unresolved => "unresolved",
    }
}

fn render_exploration_section_kind(kind: ExplorationSectionKind) -> &'static str {
    match kind {
        ExplorationSectionKind::Backlinks => "backlinks",
        ExplorationSectionKind::ForwardLinks => "forward links",
        ExplorationSectionKind::Reflinks => "reflinks",
        ExplorationSectionKind::UnlinkedReferences => "unlinked references",
        ExplorationSectionKind::TimeNeighbors => "time neighbors",
        ExplorationSectionKind::TaskNeighbors => "task neighbors",
        ExplorationSectionKind::BridgeCandidates => "bridge candidates",
        ExplorationSectionKind::DormantNotes => "dormant notes",
        ExplorationSectionKind::UnresolvedTasks => "unresolved tasks",
        ExplorationSectionKind::WeaklyIntegratedNotes => "weakly integrated notes",
    }
}

fn render_exploration_entry(output: &mut String, entry: &ExplorationEntry) {
    match entry {
        ExplorationEntry::Backlink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.source_note),
                record.row,
                record.col
            ));
            if let Some(anchor) = &record.source_anchor {
                output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
            }
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::ForwardLink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_node_identity(&record.destination_note),
                record.row,
                record.col
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Reflink { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!(
                "  matched reference: {}\n",
                record.matched_reference
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::UnlinkedReference { record } => {
            output.push_str(&format!(
                "- {} at {}:{}\n",
                render_anchor_identity(&record.source_anchor),
                record.row,
                record.col
            ));
            output.push_str(&format!("  matched text: {}\n", record.matched_text));
            output.push_str(&format!("  preview: {}\n", record.preview));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
        ExplorationEntry::Anchor { record } => {
            output.push_str(&format!("- {}\n", render_anchor_identity(&record.anchor)));
            output.push_str(&format!(
                "  why: {}\n",
                render_exploration_explanation(&record.explanation)
            ));
        }
    }
}

fn render_node_identity(node: &NodeRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        node.title, node.node_key, node.file_path, node.line
    )
}

fn render_anchor_identity(anchor: &AnchorRecord) -> String {
    format!(
        "{} [{}] {}:{}",
        anchor.title, anchor.node_key, anchor.file_path, anchor.line
    )
}

fn render_exploration_explanation(explanation: &ExplorationExplanation) -> String {
    match explanation {
        ExplorationExplanation::Backlink => "backlink".to_owned(),
        ExplorationExplanation::ForwardLink => "forward link".to_owned(),
        ExplorationExplanation::SharedReference { reference } => {
            format!("shared reference {reference}")
        }
        ExplorationExplanation::UnlinkedReference { matched_text } => {
            format!("unlinked reference text match {matched_text}")
        }
        ExplorationExplanation::TimeNeighbor { relations } => {
            format!(
                "planning relations {}",
                render_planning_relations(relations)
            )
        }
        ExplorationExplanation::TaskNeighbor {
            shared_todo_keyword,
            planning_relations,
        } => {
            let mut parts = Vec::new();
            if let Some(keyword) = shared_todo_keyword {
                parts.push(format!("shared todo {keyword}"));
            }
            if !planning_relations.is_empty() {
                parts.push(format!(
                    "planning relations {}",
                    render_planning_relations(planning_relations)
                ));
            }
            parts.join("; ")
        }
        ExplorationExplanation::BridgeCandidate {
            references,
            via_notes,
        } => format!(
            "shared references {}; via {}",
            references.join(", "),
            via_notes
                .iter()
                .map(|note| format!("{} [{}]", note.title, note.node_key))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExplorationExplanation::DormantSharedReference {
            references,
            modified_at_ns,
        } => format!(
            "shared references {}; modified_at_ns {}",
            references.join(", "),
            modified_at_ns
        ),
        ExplorationExplanation::UnresolvedSharedReference {
            references,
            todo_keyword,
        } => format!(
            "shared references {}; todo {}",
            references.join(", "),
            todo_keyword
        ),
        ExplorationExplanation::WeaklyIntegratedSharedReference {
            references,
            structural_link_count,
        } => format!(
            "shared references {}; structural link count {}",
            references.join(", "),
            structural_link_count
        ),
    }
}

fn render_planning_relations(relations: &[PlanningRelationRecord]) -> String {
    relations
        .iter()
        .map(|relation| {
            format!(
                "{}->{} {}",
                render_planning_field(relation.source_field),
                render_planning_field(relation.candidate_field),
                relation.date
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_planning_field(field: PlanningField) -> &'static str {
    match field {
        PlanningField::Scheduled => "scheduled",
        PlanningField::Deadline => "deadline",
    }
}

fn render_compare_result(result: &NoteComparisonResult, group: NoteComparisonGroup) -> String {
    let mut output = String::new();
    output.push_str(&format!("group: {}\n", render_comparison_group(group)));
    output.push_str(&format!(
        "left: {}\n",
        render_node_identity(&result.left_note)
    ));
    output.push_str(&format!(
        "right: {}\n",
        render_node_identity(&result.right_note)
    ));
    for section in &result.sections {
        output.push('\n');
        output.push_str(&format!(
            "[{}]\n",
            render_comparison_section_kind(section.kind)
        ));
        if section.entries.is_empty() {
            output.push_str("(none)\n");
            continue;
        }
        for entry in &section.entries {
            render_comparison_entry(&mut output, entry);
        }
    }
    output
}

fn render_comparison_group(group: NoteComparisonGroup) -> &'static str {
    match group {
        NoteComparisonGroup::All => "all",
        NoteComparisonGroup::Overlap => "overlap",
        NoteComparisonGroup::Divergence => "divergence",
        NoteComparisonGroup::Tension => "tension",
    }
}

fn render_comparison_section_kind(kind: NoteComparisonSectionKind) -> &'static str {
    match kind {
        NoteComparisonSectionKind::SharedRefs => "shared refs",
        NoteComparisonSectionKind::SharedPlanningDates => "shared planning dates",
        NoteComparisonSectionKind::LeftOnlyRefs => "left-only refs",
        NoteComparisonSectionKind::RightOnlyRefs => "right-only refs",
        NoteComparisonSectionKind::SharedBacklinks => "shared backlinks",
        NoteComparisonSectionKind::SharedForwardLinks => "shared forward links",
        NoteComparisonSectionKind::ContrastingTaskStates => "contrasting task states",
        NoteComparisonSectionKind::PlanningTensions => "planning tensions",
        NoteComparisonSectionKind::IndirectConnectors => "indirect connectors",
    }
}

fn render_comparison_entry(output: &mut String, entry: &NoteComparisonEntry) {
    match entry {
        NoteComparisonEntry::Reference { record } => {
            output.push_str(&format!("- {}\n", record.reference));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::Node { record } => {
            output.push_str(&format!("- {}\n", render_node_identity(&record.node)));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::PlanningRelation { record } => {
            output.push_str(&format!(
                "- {} {} <> {} {}\n",
                record.date,
                render_planning_field(record.left_field),
                render_planning_field(record.right_field),
                record.date
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
        NoteComparisonEntry::TaskState { record } => {
            output.push_str(&format!(
                "- {} <> {}\n",
                record.left_todo_keyword, record.right_todo_keyword
            ));
            output.push_str(&format!(
                "  why: {}\n",
                render_note_comparison_explanation(&record.explanation)
            ));
        }
    }
}

fn render_note_comparison_explanation(explanation: &NoteComparisonExplanation) -> String {
    match explanation {
        NoteComparisonExplanation::SharedReference => "shared reference".to_owned(),
        NoteComparisonExplanation::SharedPlanningDate => "shared planning date".to_owned(),
        NoteComparisonExplanation::LeftOnlyReference => "left-only reference".to_owned(),
        NoteComparisonExplanation::RightOnlyReference => "right-only reference".to_owned(),
        NoteComparisonExplanation::SharedBacklink => "shared backlink".to_owned(),
        NoteComparisonExplanation::SharedForwardLink => "shared forward link".to_owned(),
        NoteComparisonExplanation::ContrastingTaskState => "contrasting task state".to_owned(),
        NoteComparisonExplanation::PlanningTension => "planning tension".to_owned(),
        NoteComparisonExplanation::IndirectConnector { direction } => {
            format!(
                "indirect connector {}",
                render_connector_direction(*direction)
            )
        }
    }
}

fn render_connector_direction(direction: ComparisonConnectorDirection) -> &'static str {
    match direction {
        ComparisonConnectorDirection::LeftToRight => "left-to-right",
        ComparisonConnectorDirection::RightToLeft => "right-to-left",
        ComparisonConnectorDirection::Bidirectional => "bidirectional",
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorPayload, OutputMode, write_output};
    use slipbox_core::StatusInfo;

    #[test]
    fn writes_json_output_from_structured_results() {
        let mut output = Vec::new();
        let status = StatusInfo {
            version: "0.6.1".to_owned(),
            root: "/tmp/root".to_owned(),
            db: "/tmp/db.sqlite".to_owned(),
            files_indexed: 1,
            nodes_indexed: 2,
            links_indexed: 3,
        };

        write_output(&mut output, OutputMode::Json, &status, |_| String::new())
            .expect("json output should render");

        let parsed: StatusInfo =
            serde_json::from_slice(&output).expect("json output should deserialize");
        assert_eq!(parsed.files_indexed, 1);
    }

    #[test]
    fn writes_structured_json_errors() {
        let error = super::CliCommandError::new(OutputMode::Json, anyhow::anyhow!("broken"));
        let mut stderr = Vec::new();

        error.write(&mut stderr).expect("json error should render");

        let parsed: ErrorPayload =
            serde_json::from_slice(&stderr).expect("json error should deserialize");
        assert_eq!(parsed.error.message, "broken");
    }
}

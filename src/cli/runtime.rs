use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use slipbox_core::{
    AnchorRecord, CorpusAuditKind, ExplorationArtifactMetadata, ExplorationArtifactSummary,
    ExploreResult, NodeFromIdParams, NodeFromKeyParams, NodeFromRefParams,
    NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonResult, ReviewFindingStatus,
    ReviewRunSummary, WorkbenchPackSummary, WorkflowInputAssignment, WorkflowResolveTarget,
    WorkflowSpec, WorkflowSummary,
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
    /// Directories containing declarative workflow spec JSON files.
    #[arg(long = "workflow-dir")]
    pub(crate) workflow_dirs: Vec<PathBuf>,
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
            workflow_dirs: self.workflow_dirs.clone(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ReportFormat {
    Human,
    Json,
    Jsonl,
}

impl ReportFormat {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Jsonl => "jsonl",
        }
    }

    pub(crate) const fn error_output_mode(self) -> OutputMode {
        match self {
            Self::Human => OutputMode::Human,
            Self::Json | Self::Jsonl => OutputMode::Json,
        }
    }

    pub(crate) const fn ack_output_mode(self) -> OutputMode {
        match self {
            Self::Human => OutputMode::Human,
            Self::Json | Self::Jsonl => OutputMode::Json,
        }
    }
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
        self.connect_with_output_mode(self.output_mode())
    }

    pub(crate) fn connect_with_output_mode(
        &self,
        output_mode: OutputMode,
    ) -> Result<DaemonClient, CliCommandError> {
        let program = self
            .server_program_path()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        DaemonClient::spawn(program, &self.scope.daemon_config())
            .map_err(|error| CliCommandError::new(output_mode, error))
    }
}

pub(crate) fn normalize_daily_file_path(file_path: &str) -> Result<String, DaemonClientError> {
    let candidate = Path::new(file_path);
    if candidate.is_absolute() {
        return Err(invalid_request_error(
            "daily file path must be relative to --root",
        ));
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_request_error(
                    "daily file path must stay within --root",
                ));
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        return Err(invalid_request_error("daily file path must not be empty"));
    }
    if !normalized.ends_with(".org") {
        return Err(invalid_request_error("daily file path must end with .org"));
    }
    Ok(normalized)
}

pub(crate) fn normalize_edit_file_path(
    root: &Path,
    file_path: &Path,
) -> Result<String, DaemonClientError> {
    let normalized = normalize_root_relative_path(root, file_path, "edit file path")?;
    if !normalized.ends_with(".org") {
        return Err(invalid_request_error("edit file path must end with .org"));
    }
    Ok(normalized)
}

pub(crate) fn normalize_diagnostic_file_path(
    root: &Path,
    file_path: &Path,
) -> Result<String, DaemonClientError> {
    normalize_root_relative_path(root, file_path, "diagnostic file path")
}

pub(crate) fn normalize_root_relative_path(
    root: &Path,
    file_path: &Path,
    description: &str,
) -> Result<String, DaemonClientError> {
    let relative = if file_path.is_absolute() {
        let absolute_root = canonical_edit_root(root)?;
        file_path.strip_prefix(&absolute_root).map_err(|_| {
            invalid_request_error(format!(
                "{description} must stay within --root: {}",
                file_path.display()
            ))
        })?
    } else {
        file_path
    };

    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_request_error(format!(
                    "{description} must stay within --root"
                )));
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        return Err(invalid_request_error(format!(
            "{description} must not be empty"
        )));
    }
    Ok(normalized)
}

fn canonical_edit_root(root: &Path) -> Result<PathBuf, DaemonClientError> {
    root.canonicalize()
        .map_err(|error| invalid_request_error(format!("failed to resolve --root: {error}")))
}

pub(crate) fn validate_region_range(start: u32, end: u32) -> Result<(), DaemonClientError> {
    if start == 0 || end == 0 {
        return Err(invalid_request_error(
            "edit region positions must be positive 1-based character positions",
        ));
    }
    if start == end {
        return Err(invalid_request_error(
            "active region range must not be empty",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Args)]
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

#[derive(Debug, Clone, Args, Default)]
pub(crate) struct ReportOutputArgs {
    /// Write the rendered report to this path instead of stdout. Use `-` for stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
    /// Emit line-oriented JSON instead of a single structured result document.
    #[arg(long)]
    pub(crate) jsonl: bool,
}

impl ReportOutputArgs {
    pub(crate) fn format(&self, output_mode: OutputMode) -> Result<ReportFormat> {
        if self.jsonl && matches!(output_mode, OutputMode::Json) {
            anyhow::bail!("--json and --jsonl are mutually exclusive");
        }

        Ok(if self.jsonl {
            ReportFormat::Jsonl
        } else if matches!(output_mode, OutputMode::Json) {
            ReportFormat::Json
        } else {
            ReportFormat::Human
        })
    }

    pub(crate) fn output_path(&self) -> Option<&Path> {
        self.output
            .as_deref()
            .filter(|path| *path != Path::new("-"))
    }
}

#[derive(Debug, Clone, Args, Default)]
pub(crate) struct SaveReviewArgs {
    /// Persist this live audit or workflow run as a durable review.
    #[arg(long = "save-review")]
    pub(crate) save_review: bool,
    /// Durable identifier to assign to the saved review.
    #[arg(long = "review-id")]
    pub(crate) review_id: Option<String>,
    /// Human title to assign to the saved review.
    #[arg(long = "review-title")]
    pub(crate) review_title: Option<String>,
    /// Optional human summary for the saved review.
    #[arg(long = "review-summary")]
    pub(crate) review_summary: Option<String>,
    /// Replace an existing saved review with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SaveArtifactArgs {
    /// Persist the live command as a durable exploration artifact.
    #[arg(long)]
    pub(crate) save: bool,
    /// Durable identifier to assign to the saved artifact.
    #[arg(long = "artifact-id")]
    pub(crate) artifact_id: Option<String>,
    /// Human title to assign to the saved artifact.
    #[arg(long = "artifact-title")]
    pub(crate) artifact_title: Option<String>,
    /// Optional human summary for the saved artifact.
    #[arg(long = "artifact-summary")]
    pub(crate) artifact_summary: Option<String>,
    /// Replace an existing saved artifact with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

pub(crate) fn report_error(error: &CliCommandError) -> ExitCode {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = error.write(&mut writer);
    ExitCode::from(1)
}

#[derive(Debug, Serialize)]
pub(crate) struct ArtifactExportFileResult {
    pub(crate) artifact: ExplorationArtifactSummary,
    pub(crate) output_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct PackExportFileResult {
    pub(crate) pack: WorkbenchPackSummary,
    pub(crate) output_path: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct GraphDotFileResult {
    pub(crate) output_path: String,
    pub(crate) format: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct SavedExploreCommandResult {
    pub(crate) result: ExploreResult,
    pub(crate) artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct SavedCompareCommandResult {
    pub(crate) result: NoteComparisonResult,
    pub(crate) artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkflowReportOutputResult {
    pub(crate) workflow: WorkflowSummary,
    pub(crate) format: ReportFormat,
    pub(crate) output_path: String,
    pub(crate) step_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct SavedWorkflowReportOutputResult {
    pub(crate) workflow: WorkflowSummary,
    pub(crate) format: ReportFormat,
    pub(crate) output_path: String,
    pub(crate) step_count: usize,
    pub(crate) review: ReviewRunSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct AuditReportOutputResult {
    pub(crate) audit: CorpusAuditKind,
    pub(crate) format: ReportFormat,
    pub(crate) output_path: String,
    pub(crate) entry_count: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct SavedAuditReportOutputResult {
    pub(crate) audit: CorpusAuditKind,
    pub(crate) format: ReportFormat,
    pub(crate) output_path: String,
    pub(crate) entry_count: usize,
    pub(crate) review: ReviewRunSummary,
}

#[derive(Debug, Serialize)]
pub(crate) struct WorkflowShowFileResult {
    pub(crate) workflow: WorkflowSpec,
}

#[derive(Debug, Clone)]
pub(crate) struct SaveReviewRequest {
    pub(crate) review_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) summary: Option<String>,
    pub(crate) overwrite: bool,
}

impl SaveReviewArgs {
    pub(crate) fn request(&self) -> Result<Option<SaveReviewRequest>> {
        let mut stray_flags = Vec::new();
        if self.review_id.is_some() {
            stray_flags.push("--review-id");
        }
        if self.review_title.is_some() {
            stray_flags.push("--review-title");
        }
        if self.review_summary.is_some() {
            stray_flags.push("--review-summary");
        }
        if self.overwrite {
            stray_flags.push("--overwrite");
        }

        if !self.save_review {
            if stray_flags.is_empty() {
                return Ok(None);
            }
            anyhow::bail!("{} require --save-review", render_flag_list(&stray_flags));
        }

        Ok(Some(SaveReviewRequest {
            review_id: self.review_id.clone(),
            title: self.review_title.clone(),
            summary: self.review_summary.clone(),
            overwrite: self.overwrite,
        }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SaveArtifactRequest {
    pub(crate) metadata: ExplorationArtifactMetadata,
    pub(crate) overwrite: bool,
}

impl SaveArtifactArgs {
    pub(crate) fn request(&self) -> Result<Option<SaveArtifactRequest>> {
        let mut stray_flags = Vec::new();
        if self.artifact_id.is_some() {
            stray_flags.push("--artifact-id");
        }
        if self.artifact_title.is_some() {
            stray_flags.push("--artifact-title");
        }
        if self.artifact_summary.is_some() {
            stray_flags.push("--artifact-summary");
        }
        if self.overwrite {
            stray_flags.push("--overwrite");
        }

        if !self.save {
            if stray_flags.is_empty() {
                return Ok(None);
            }
            anyhow::bail!("{} require --save", render_flag_list(&stray_flags));
        }

        let mut missing_flags = Vec::new();
        if self.artifact_id.is_none() {
            missing_flags.push("--artifact-id");
        }
        if self.artifact_title.is_none() {
            missing_flags.push("--artifact-title");
        }
        if !missing_flags.is_empty() {
            anyhow::bail!("--save requires {}", render_flag_list(&missing_flags));
        }

        Ok(Some(SaveArtifactRequest {
            metadata: ExplorationArtifactMetadata {
                artifact_id: self
                    .artifact_id
                    .clone()
                    .expect("validated artifact_id should be present"),
                title: self
                    .artifact_title
                    .clone()
                    .expect("validated artifact_title should be present"),
                summary: self.artifact_summary.clone(),
            },
            overwrite: self.overwrite,
        }))
    }
}

pub(crate) trait HeadlessCommand {
    type Output: Serialize;

    fn headless_args(&self) -> &HeadlessArgs;
    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError>;
    fn render_human(&self, output: &Self::Output) -> String;
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

    pub(crate) fn write(&self, writer: &mut impl Write) -> Result<()> {
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
pub(crate) struct ErrorPayload {
    pub(crate) error: ErrorMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ErrorMessage {
    pub(crate) message: String,
}

pub(crate) fn run_headless_command<C>(command: &C) -> Result<(), CliCommandError>
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

pub(crate) fn write_output<T>(
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

pub(crate) fn render_report_bytes<T, L>(
    format: ReportFormat,
    value: &T,
    human_renderer: impl FnOnce(&T) -> String,
    jsonl_renderer: impl FnOnce(&T) -> Vec<L>,
) -> Result<Vec<u8>>
where
    T: Serialize,
    L: Serialize,
{
    let mut bytes = Vec::new();
    match format {
        ReportFormat::Human => bytes.extend_from_slice(human_renderer(value).as_bytes()),
        ReportFormat::Json => {
            serde_json::to_writer(&mut bytes, value)?;
            bytes.push(b'\n');
        }
        ReportFormat::Jsonl => {
            for line in jsonl_renderer(value) {
                serde_json::to_writer(&mut bytes, &line)?;
                bytes.push(b'\n');
            }
        }
    }
    Ok(bytes)
}

pub(crate) fn write_report_destination(bytes: &[u8], output_path: Option<&Path>) -> Result<()> {
    if let Some(path) = output_path {
        fs::write(path, bytes)
            .with_context(|| format!("failed to write report to {}", path.display()))?;
    } else {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        writer.write_all(bytes)?;
        writer.flush()?;
    }
    Ok(())
}

pub(crate) fn parse_review_finding_status(value: &str) -> Result<ReviewFindingStatus> {
    match value {
        "open" => Ok(ReviewFindingStatus::Open),
        "reviewed" => Ok(ReviewFindingStatus::Reviewed),
        "dismissed" => Ok(ReviewFindingStatus::Dismissed),
        "accepted" => Ok(ReviewFindingStatus::Accepted),
        _ => anyhow::bail!(
            "invalid review finding status `{value}`; expected one of: open, reviewed, dismissed, accepted"
        ),
    }
}

pub(crate) fn parse_workflow_input_assignments(
    values: &[String],
) -> Result<Vec<WorkflowInputAssignment>, DaemonClientError> {
    values
        .iter()
        .map(|value| parse_workflow_input_assignment(value))
        .collect()
}

fn parse_workflow_input_assignment(
    value: &str,
) -> Result<WorkflowInputAssignment, DaemonClientError> {
    let (input_id, encoded_target) = value.split_once('=').ok_or_else(|| {
        DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(format!(
            "invalid workflow input assignment {value}: expected input-id=kind:value"
        )))
    })?;
    let (kind, target_value) = encoded_target.split_once(':').ok_or_else(|| {
        DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(format!(
            "invalid workflow input assignment {value}: expected input-id=kind:value"
        )))
    })?;
    if input_id.trim().is_empty() || target_value.trim().is_empty() {
        return Err(DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(
            format!(
                "invalid workflow input assignment {value}: expected non-empty input id and target value"
            ),
        )));
    }

    let target = match kind {
        "id" => WorkflowResolveTarget::Id {
            id: target_value.to_owned(),
        },
        "title" => WorkflowResolveTarget::Title {
            title: target_value.to_owned(),
        },
        "ref" => WorkflowResolveTarget::Reference {
            reference: target_value.to_owned(),
        },
        "key" => WorkflowResolveTarget::NodeKey {
            node_key: target_value.to_owned(),
        },
        _ => {
            return Err(DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(
                format!("invalid workflow input assignment {value}: unknown target kind {kind}"),
            )));
        }
    };

    Ok(WorkflowInputAssignment {
        input_id: input_id.to_owned(),
        target,
    })
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

pub(crate) fn resolve_anchor_or_note_target_key(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<String, DaemonClientError> {
    match target {
        ResolveTarget::Key(node_key) => Ok(node_key.clone()),
        _ => resolve_note_target(client, target).map(|node| node.node_key),
    }
}

pub(crate) fn require_resolved_node(
    node: Option<NodeRecord>,
    error_message: String,
) -> Result<NodeRecord, DaemonClientError> {
    node.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

pub(crate) fn require_resolved_anchor(
    anchor: Option<AnchorRecord>,
    error_message: String,
) -> Result<AnchorRecord, DaemonClientError> {
    anchor.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

pub(crate) fn invalid_request_error(message: impl Into<String>) -> DaemonClientError {
    DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(message.into()))
}

pub(crate) fn render_flag_list(flags: &[&str]) -> String {
    match flags {
        [] => String::new(),
        [flag] => (*flag).to_owned(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            let mut output = flags[..flags.len() - 1].join(", ");
            output.push_str(", and ");
            output.push_str(flags[flags.len() - 1]);
            output
        }
    }
}

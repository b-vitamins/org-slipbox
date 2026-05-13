use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use slipbox_core::{
    AgendaParams, AgendaResult, AnchorRecord, AppendHeadingAtOutlinePathParams,
    AppendHeadingParams, AppendHeadingToNodeParams, AppliedReportProfile,
    AuditRemediationApplyAction, AuditRemediationConfidence, AuditRemediationPreviewPayload,
    BacklinksParams, BacklinksResult, CaptureContentType, CaptureNodeParams, CaptureTemplateParams,
    CaptureTemplatePreviewParams, CaptureTemplatePreviewResult, CompareNotesParams,
    ComparisonConnectorDirection, CorpusAuditEntry, CorpusAuditKind, CorpusAuditParams,
    CorpusAuditResult, DeleteExplorationArtifactResult, DeleteReviewRunResult,
    DeleteWorkbenchPackResult, EnsureFileNodeParams, EnsureNodeIdParams,
    ExecuteExplorationArtifactResult, ExecutedExplorationArtifact,
    ExecutedExplorationArtifactPayload, ExplorationArtifactIdParams, ExplorationArtifactKind,
    ExplorationArtifactMetadata, ExplorationArtifactPayload, ExplorationArtifactResult,
    ExplorationArtifactSummary, ExplorationEntry, ExplorationExplanation, ExplorationLens,
    ExplorationSectionKind, ExploreParams, ExploreResult, ExtractSubtreeParams, FileRecord,
    ForwardLinksParams, ForwardLinksResult, GraphParams, GraphResult, GraphTitleShortening,
    ImportWorkbenchPackParams, IndexFileParams, IndexFileResult, IndexStats, IndexedFilesResult,
    ListExplorationArtifactsResult, ListReviewRoutinesResult, ListReviewRunsResult,
    ListWorkbenchPacksResult, ListWorkflowsResult, MarkReviewFindingParams,
    MarkReviewFindingResult, NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams,
    NodeFromRefParams, NodeFromTitleOrAliasParams, NodeRecord, NoteComparisonEntry,
    NoteComparisonExplanation, NoteComparisonGroup, NoteComparisonResult,
    NoteComparisonSectionKind, OccurrenceRecord, PlanningField, PlanningRelationRecord,
    RandomNodeResult, RefRecord, RefileRegionParams, RefileSubtreeParams, ReviewFinding,
    ReviewFindingKind, ReviewFindingPair, ReviewFindingPayload,
    ReviewFindingRemediationApplyParams, ReviewFindingRemediationApplyResult,
    ReviewFindingRemediationPreview, ReviewFindingRemediationPreviewParams,
    ReviewFindingRemediationPreviewResult, ReviewFindingStatus, ReviewFindingStatusDiff,
    ReviewRoutineCompareResult, ReviewRoutineExecutionResult, ReviewRoutineIdParams,
    ReviewRoutineReportLine, ReviewRoutineResult, ReviewRoutineSource,
    ReviewRoutineSourceExecutionResult, ReviewRoutineSpec, ReviewRoutineSummary, ReviewRun,
    ReviewRunDiff, ReviewRunDiffParams, ReviewRunDiffResult, ReviewRunIdParams, ReviewRunKind,
    ReviewRunPayload, ReviewRunResult, ReviewRunSummary, RewriteFileParams, RunReviewRoutineParams,
    RunReviewRoutineResult, RunWorkflowParams, RunWorkflowResult, SaveCorpusAuditReviewParams,
    SaveCorpusAuditReviewResult, SaveExplorationArtifactParams, SaveWorkflowReviewParams,
    SaveWorkflowReviewResult, SavedComparisonArtifact, SavedExplorationArtifact,
    SavedLensViewArtifact, SavedTrailArtifact, SavedTrailStep, SearchFilesParams,
    SearchFilesResult, SearchNodesParams, SearchNodesResult, SearchOccurrencesParams,
    SearchOccurrencesResult, SearchRefsParams, SearchRefsResult, SearchTagsParams,
    SearchTagsResult, SlipboxLinkRewriteApplyParams, SlipboxLinkRewriteApplyResult,
    SlipboxLinkRewritePreview, SlipboxLinkRewritePreviewParams, SlipboxLinkRewritePreviewResult,
    StatusInfo, StructuralWriteReport, StructuralWriteResult, TrailReplayResult,
    TrailReplayStepResult, UpdateNodeMetadataParams, ValidateWorkbenchPackResult,
    WorkbenchPackCompatibilityEnvelope, WorkbenchPackIdParams, WorkbenchPackIssue,
    WorkbenchPackIssueKind, WorkbenchPackManifest, WorkbenchPackResult, WorkbenchPackSummary,
    WorkflowArtifactSaveSource, WorkflowCatalogIssue, WorkflowExecutionResult,
    WorkflowExploreFocus, WorkflowIdParams, WorkflowInputAssignment, WorkflowInputKind,
    WorkflowInputSpec, WorkflowResolveTarget, WorkflowSpec, WorkflowSpecCompatibilityEnvelope,
    WorkflowStepPayload, WorkflowStepReport, WorkflowStepReportPayload, WorkflowStepSpec,
    WorkflowSummary,
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
enum ReportFormat {
    Human,
    Json,
    Jsonl,
}

impl ReportFormat {
    const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Jsonl => "jsonl",
        }
    }

    const fn error_output_mode(self) -> OutputMode {
        match self {
            Self::Human => OutputMode::Human,
            Self::Json | Self::Jsonl => OutputMode::Json,
        }
    }

    const fn ack_output_mode(self) -> OutputMode {
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

#[derive(Debug, Clone, Args)]
pub(crate) struct StatusArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncArgs {
    #[command(subcommand)]
    pub(crate) command: SyncCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum SyncCommand {
    /// Refresh the full indexed root through daemon-owned discovery.
    Root(SyncRootArgs),
    /// Refresh one file's indexed state without pruning the rest of the root.
    File(SyncFileArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncRootArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct SyncFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to refresh, absolute or relative to --root.
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileArgs {
    #[command(subcommand)]
    pub(crate) command: FileCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum FileCommand {
    /// List indexed files.
    List(FileListArgs),
    /// Search indexed file paths and titles.
    Search(FileSearchArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct FileSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Search query matched against indexed file paths and titles.
    pub(crate) query: String,
    /// Maximum file records to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

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
    /// Search query matched against indexed note titles, aliases, refs, and text.
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
    #[arg(long)]
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
    #[arg(required = true)]
    pub(crate) values: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NodeMetadataSetArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Complete metadata values to keep. Omit all values to clear the list.
    pub(crate) values: Vec<String>,
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
    Show(RefShowArgs),
    /// Resolve one reference to its note.
    Resolve(RefShowArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RefSearchArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Reference query.
    pub(crate) query: String,
    /// Maximum reference records to return.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RefShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Reference to resolve.
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
    pub(crate) start: String,
    /// Inclusive ISO end date, YYYY-MM-DD.
    pub(crate) end: String,
    /// Maximum agenda entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyArgs {
    #[command(subcommand)]
    pub(crate) command: DailyCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DailyCommand {
    /// Ensure a daily note exists.
    Ensure(DailyEnsureArgs),
    /// Show an already indexed daily note without creating it.
    Show(DailyShowArgs),
    /// Append a heading to a daily note.
    Append(DailyAppendArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyTargetArgs {
    /// ISO date to use, YYYY-MM-DD. Defaults to today's local date.
    #[arg(long)]
    pub(crate) date: Option<String>,
    /// Daily note directory inside --root.
    #[arg(long, default_value = "daily")]
    pub(crate) directory: String,
    /// strftime-compatible daily note filename format.
    #[arg(long = "file-format", default_value = "%Y-%m-%d.org")]
    pub(crate) file_format: String,
    /// strftime-compatible daily note title format.
    #[arg(long = "title-format", default_value = "%Y-%m-%d")]
    pub(crate) title_format: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyEnsureArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long)]
    pub(crate) head: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyShowArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyAppendArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
    /// Heading title to append.
    #[arg(long)]
    pub(crate) heading: String,
    /// Org heading level.
    #[arg(long, default_value_t = 1)]
    pub(crate) level: u32,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long)]
    pub(crate) head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DailyTarget {
    date: NaiveDate,
    file_path: String,
    title: String,
}

impl DailyTarget {
    fn node_key(&self) -> String {
        format!("file:{}", self.file_path.replace('\\', "/"))
    }
}

impl DailyTargetArgs {
    fn target(&self) -> Result<DailyTarget, DaemonClientError> {
        if Path::new(&self.directory).is_absolute() {
            return Err(invalid_request_error(
                "daily --directory must be relative to --root",
            ));
        }
        let date = match &self.date {
            Some(date) => parse_daily_date(date)?,
            None => today_local_date(),
        };
        let filename = date.format(&self.file_format).to_string();
        let file_path = if self.directory.trim().is_empty() {
            filename
        } else {
            PathBuf::from(&self.directory)
                .join(filename)
                .display()
                .to_string()
        };
        Ok(DailyTarget {
            date,
            file_path: normalize_daily_file_path(&file_path)?,
            title: date.format(&self.title_format).to_string(),
        })
    }
}

fn normalize_daily_file_path(file_path: &str) -> Result<String, DaemonClientError> {
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

fn normalize_edit_file_path(root: &Path, file_path: &Path) -> Result<String, DaemonClientError> {
    let relative = if file_path.is_absolute() {
        let absolute_root = canonical_edit_root(root)?;
        file_path.strip_prefix(&absolute_root).map_err(|_| {
            invalid_request_error(format!(
                "edit file path must stay within --root: {}",
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
                return Err(invalid_request_error(
                    "edit file path must stay within --root",
                ));
            }
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.is_empty() {
        return Err(invalid_request_error("edit file path must not be empty"));
    }
    if !normalized.ends_with(".org") {
        return Err(invalid_request_error("edit file path must end with .org"));
    }
    Ok(normalized)
}

fn canonical_edit_root(root: &Path) -> Result<PathBuf, DaemonClientError> {
    root.canonicalize()
        .map_err(|error| invalid_request_error(format!("failed to resolve --root: {error}")))
}

fn validate_region_range(start: u32, end: u32) -> Result<(), DaemonClientError> {
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
    #[arg(long)]
    pub(crate) root_node_key: Option<String>,
    /// Maximum graph distance from --root-node-key.
    #[arg(long)]
    pub(crate) max_distance: Option<u32>,
    /// Include disconnected notes in global graph output.
    #[arg(long)]
    pub(crate) include_orphans: bool,
    /// Link type to hide. Only `id` is currently supported.
    #[arg(long = "hide-link-type")]
    pub(crate) hidden_link_types: Vec<String>,
    /// Maximum graph label length before shortening.
    #[arg(long, default_value_t = 100)]
    pub(crate) max_title_length: usize,
    /// Title shortening mode.
    #[arg(long, value_enum)]
    pub(crate) shorten_titles: Option<GraphShortenArg>,
    /// URL prefix for nodes with explicit IDs.
    #[arg(long)]
    pub(crate) node_url_prefix: Option<String>,
    /// Write DOT to this path instead of stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureArgs {
    #[command(subcommand)]
    pub(crate) command: CaptureCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CaptureCommand {
    /// Capture a file note.
    Node(CaptureNodeCommandArgs),
    /// Capture content through the Rust capture-template engine.
    Template(CaptureTemplateCommandArgs),
    /// Preview capture-template output without writing files.
    Preview(CapturePreviewCommandArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum CaptureTypeArg {
    Plain,
    Entry,
    Item,
    Checkitem,
    TableLine,
}

impl From<CaptureTypeArg> for CaptureContentType {
    fn from(value: CaptureTypeArg) -> Self {
        match value {
            CaptureTypeArg::Plain => Self::Plain,
            CaptureTypeArg::Entry => Self::Entry,
            CaptureTypeArg::Item => Self::Item,
            CaptureTypeArg::Checkitem => Self::Checkitem,
            CaptureTypeArg::TableLine => Self::TableLine,
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureNodeCommandArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Note title.
    #[arg(long)]
    pub(crate) title: String,
    /// File path to create, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: Option<PathBuf>,
    /// Optional leading Org content to preserve before generated metadata.
    #[arg(long)]
    pub(crate) head: Option<String>,
    /// Reference to attach to the created note.
    #[arg(long = "ref", num_args = 1..)]
    pub(crate) refs: Vec<String>,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("content-source")
        .args(["content", "content_file", "content_stdin"])
        .multiple(false)
))]
pub(crate) struct CaptureTemplateFields {
    /// Optional title used by file creation and entry captures.
    #[arg(long, default_value = "")]
    pub(crate) title: String,
    /// File path target, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: Option<PathBuf>,
    /// Exact target node key.
    #[arg(long = "node-key")]
    pub(crate) node_key: Option<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long)]
    pub(crate) head: Option<String>,
    /// One or more outline path segments.
    #[arg(long = "outline", num_args = 1..)]
    pub(crate) outline_path: Vec<String>,
    /// Capture content type.
    #[arg(long = "type", value_enum, default_value_t = CaptureTypeArg::Plain)]
    pub(crate) capture_type: CaptureTypeArg,
    /// Capture content supplied directly on the command line.
    #[arg(long)]
    pub(crate) content: Option<String>,
    /// Read capture content from a file.
    #[arg(long = "content-file")]
    pub(crate) content_file: Option<PathBuf>,
    /// Read capture content from stdin.
    #[arg(long = "content-stdin")]
    pub(crate) content_stdin: bool,
    /// Reference to attach when creating a new target file.
    #[arg(long = "ref", num_args = 1..)]
    pub(crate) refs: Vec<String>,
    /// Insert before existing target body content when supported.
    #[arg(long)]
    pub(crate) prepend: bool,
    /// Empty lines before inserted capture content.
    #[arg(long, default_value_t = 0)]
    pub(crate) empty_lines_before: u32,
    /// Empty lines after inserted capture content.
    #[arg(long, default_value_t = 0)]
    pub(crate) empty_lines_after: u32,
    /// Org table insertion position used for table-line captures.
    #[arg(long = "table-line-pos")]
    pub(crate) table_line_pos: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureTemplateCommandArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) template: CaptureTemplateFields,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("preview-source")
        .args(["source_override", "source_file"])
        .multiple(false)
))]
pub(crate) struct CapturePreviewCommandArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) template: CaptureTemplateFields,
    /// Source text to preview against instead of reading the target file.
    #[arg(long = "source")]
    pub(crate) source_override: Option<String>,
    /// Read source text to preview against from a file.
    #[arg(long = "source-file")]
    pub(crate) source_file: Option<PathBuf>,
    /// Ensure the previewed target node has an explicit ID in rendered output.
    #[arg(long)]
    pub(crate) ensure_node_id: bool,
}

impl CaptureTemplateFields {
    fn params(&self) -> Result<CaptureTemplateParams> {
        self.validate_target_mode()?;
        Ok(CaptureTemplateParams {
            title: self.title.clone(),
            file_path: self.file.as_ref().map(|path| path.display().to_string()),
            node_key: self.node_key.clone(),
            head: self.head.clone(),
            outline_path: self.outline_path.clone(),
            capture_type: self.capture_type.into(),
            content: self.content()?,
            refs: self.refs.clone(),
            prepend: self.prepend,
            empty_lines_before: self.empty_lines_before,
            empty_lines_after: self.empty_lines_after,
            table_line_pos: self.table_line_pos.clone(),
        })
    }

    fn content(&self) -> Result<String> {
        if let Some(content) = &self.content {
            return Ok(content.clone());
        }
        if let Some(path) = &self.content_file {
            return fs::read_to_string(path).with_context(|| {
                format!("failed to read capture content from {}", path.display())
            });
        }
        if self.content_stdin {
            let mut content = String::new();
            io::stdin()
                .read_to_string(&mut content)
                .context("failed to read capture content from stdin")?;
            return Ok(content);
        }
        Ok(String::new())
    }

    fn validate_target_mode(&self) -> Result<()> {
        if self.node_key.is_some()
            && (self.file.is_some()
                || self.head.is_some()
                || !self.outline_path.is_empty()
                || !self.refs.is_empty())
        {
            anyhow::bail!("--node-key cannot be combined with --file, --head, --outline, or --ref");
        }
        Ok(())
    }
}

impl CapturePreviewCommandArgs {
    fn source_override(&self) -> Result<Option<String>> {
        if let Some(source) = &self.source_override {
            return Ok(Some(source.clone()));
        }
        if let Some(path) = &self.source_file {
            return fs::read_to_string(path)
                .with_context(|| {
                    format!(
                        "failed to read capture preview source from {}",
                        path.display()
                    )
                })
                .map(Some);
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteArgs {
    #[command(subcommand)]
    pub(crate) command: NoteCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum NoteCommand {
    /// Create a file note.
    Create(NoteCreateArgs),
    /// Ensure a specific file note exists.
    EnsureFile(NoteEnsureFileArgs),
    /// Append a heading to a file note.
    AppendHeading(NoteAppendHeadingArgs),
    /// Append a child heading below an exact note target.
    AppendToNode(NoteAppendToNodeArgs),
    /// Append a heading under an outline path, creating missing outline headings.
    AppendOutline(NoteAppendOutlineArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteCreateArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Note title.
    #[arg(long)]
    pub(crate) title: String,
    /// File path to create, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: Option<PathBuf>,
    /// Optional leading Org content to preserve before generated metadata.
    #[arg(long)]
    pub(crate) head: Option<String>,
    /// Reference to attach to the created note.
    #[arg(long = "ref", num_args = 1..)]
    pub(crate) refs: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteEnsureFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to ensure, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: PathBuf,
    /// File note title.
    #[arg(long)]
    pub(crate) title: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendHeadingArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: PathBuf,
    /// File note title used when the file must be created.
    #[arg(long)]
    pub(crate) title: String,
    /// Heading title to append.
    #[arg(long)]
    pub(crate) heading: String,
    /// Org heading level.
    #[arg(long, default_value_t = 1)]
    pub(crate) level: u32,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendToNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
    /// Child heading title to append.
    #[arg(long)]
    pub(crate) heading: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendOutlineArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: PathBuf,
    /// Heading title to append under the outline path.
    #[arg(long)]
    pub(crate) heading: String,
    /// One or more outline path segments.
    #[arg(long = "outline", required = true, num_args = 1..)]
    pub(crate) outline_path: Vec<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long)]
    pub(crate) head: Option<String>,
}

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
    #[arg(long)]
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
    #[arg(long)]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditPromoteFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to rewrite, absolute or relative to --root.
    #[arg(long)]
    pub(crate) file: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct EditDemoteFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to rewrite, absolute or relative to --root.
    #[arg(long)]
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
    #[arg(long = "source-id", group = "source-target")]
    pub(crate) source_id: Option<String>,
    /// Resolve the source anchor by exact title or alias.
    #[arg(long = "source-title", group = "source-target")]
    pub(crate) source_title: Option<String>,
    /// Resolve the source anchor by exact reference.
    #[arg(long = "source-ref", group = "source-target")]
    pub(crate) source_reference: Option<String>,
    /// Use an exact source node key. This may be an anonymous heading anchor.
    #[arg(long = "source-key", group = "source-target")]
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
    #[arg(long = "target-id", group = "edit-target")]
    pub(crate) target_id: Option<String>,
    /// Resolve the target note by exact title or alias.
    #[arg(long = "target-title", group = "edit-target")]
    pub(crate) target_title: Option<String>,
    /// Resolve the target note by exact reference.
    #[arg(long = "target-ref", group = "edit-target")]
    pub(crate) target_reference: Option<String>,
    /// Resolve the target note by exact node key.
    #[arg(long = "target-key", group = "edit-target")]
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
    Apply(LinkRewriteSlipboxApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct LinkRewriteSlipboxFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to inspect or rewrite, absolute or relative to --root.
    #[arg(long)]
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
pub(crate) struct AuditArgs {
    #[command(subcommand)]
    pub(crate) command: AuditCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum AuditCommand {
    /// List links that point to missing explicit IDs.
    DanglingLinks(AuditRunArgs),
    /// Group note-title collisions that may need disambiguation.
    DuplicateTitles(AuditRunArgs),
    /// List notes with no refs, backlinks, or outgoing links.
    OrphanNotes(AuditRunArgs),
    /// List ref-backed notes with very weak structural integration.
    WeaklyIntegratedNotes(AuditRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct AuditRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Maximum audit entries to return.
    #[arg(long, default_value_t = 200)]
    pub(crate) limit: usize,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowArgs {
    #[command(subcommand)]
    pub(crate) command: WorkflowCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum WorkflowCommand {
    /// List available named workflows.
    List(WorkflowListArgs),
    /// Show a built-in workflow or inspect a workflow spec JSON file/stdin.
    Show(WorkflowShowArgs),
    /// Run a built-in workflow over the canonical daemon boundary.
    Run(WorkflowRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
#[command(group(
    ArgGroup::new("workflow-source")
        .args(["workflow_id", "spec"])
        .required(true)
        .multiple(false)
))]
pub(crate) struct WorkflowShowArgs {
    /// Root directory containing Org files when showing a built-in workflow through the daemon.
    #[arg(long)]
    pub(crate) root: Option<PathBuf>,
    /// SQLite database path when showing a built-in workflow through the daemon.
    #[arg(long)]
    pub(crate) db: Option<PathBuf>,
    /// Directories containing declarative workflow spec JSON files.
    #[arg(long = "workflow-dir")]
    pub(crate) workflow_dirs: Vec<PathBuf>,
    /// File extensions eligible for discovery and indexing.
    #[arg(long = "file-extension")]
    pub(crate) file_extensions: Vec<String>,
    /// Relative-path regular expressions to exclude from discovery.
    #[arg(long = "exclude-regexp")]
    pub(crate) exclude_regexps: Vec<String>,
    /// Path to the slipbox executable used to spawn `slipbox serve`.
    #[arg(long)]
    pub(crate) server_program: Option<PathBuf>,
    /// Emit structured JSON to stdout and structured errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
    /// Built-in workflow identifier to inspect through the daemon.
    #[arg(group = "workflow-source")]
    pub(crate) workflow_id: Option<String>,
    /// Read workflow spec JSON from this path, or `-` for stdin.
    #[arg(long, group = "workflow-source")]
    pub(crate) spec: Option<String>,
}

impl WorkflowShowArgs {
    #[must_use]
    fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }

    fn headless_args(&self) -> Result<HeadlessArgs> {
        let root = self
            .root
            .clone()
            .context("workflow show for built-in workflows requires --root and --db")?;
        let db = self
            .db
            .clone()
            .context("workflow show for built-in workflows requires --root and --db")?;
        Ok(HeadlessArgs {
            scope: ScopeArgs {
                root,
                db,
                workflow_dirs: self.workflow_dirs.clone(),
                file_extensions: self.file_extensions.clone(),
                exclude_regexps: self.exclude_regexps.clone(),
            },
            server_program: self.server_program.clone(),
            json: self.json,
        })
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct WorkflowRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Built-in workflow identifier to run.
    pub(crate) workflow_id: String,
    /// Workflow input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input")]
    pub(crate) inputs: Vec<String>,
    #[command(flatten)]
    pub(crate) report: ReportOutputArgs,
    #[command(flatten)]
    pub(crate) save_review: SaveReviewArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineArgs {
    #[command(subcommand)]
    pub(crate) command: RoutineCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum RoutineCommand {
    /// List available review routines.
    List(RoutineListArgs),
    /// Show a review routine definition.
    Show(RoutineShowArgs),
    /// Run a review routine through daemon-owned semantics.
    Run(RoutineRunArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review routine identifier.
    pub(crate) routine_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineShowArgs {
    #[command(flatten)]
    pub(crate) routine: RoutineIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct RoutineRunArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Review routine identifier to run.
    pub(crate) routine_id: String,
    /// Routine input assignment as `input-id=kind:value` where kind is `id`, `title`, `ref`, or `key`.
    #[arg(long = "input")]
    pub(crate) inputs: Vec<String>,
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
    fn format(&self, output_mode: OutputMode) -> Result<ReportFormat> {
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

    fn output_path(&self) -> Option<&Path> {
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

#[derive(Debug, Clone, Args)]
pub(crate) struct PackArgs {
    #[command(subcommand)]
    pub(crate) command: PackCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum PackCommand {
    /// List imported workbench packs and catalog issues.
    List(PackListArgs),
    /// Show an imported workbench pack manifest.
    Show(PackShowArgs),
    /// Validate a local workbench pack JSON file/stdin without daemon state.
    Validate(PackValidateArgs),
    /// Import a workbench pack manifest through the daemon.
    Import(PackImportArgs),
    /// Export an imported workbench pack manifest as stable JSON.
    Export(PackExportArgs),
    /// Delete an imported workbench pack by durable identifier.
    Delete(PackDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable workbench pack identifier.
    pub(crate) pack_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackShowArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackValidateArgs {
    /// Read workbench pack JSON from this path, or `-` for stdin.
    #[arg(default_value = "-")]
    pub(crate) input: String,
    /// Emit structured JSON to stdout and structured errors to stderr.
    #[arg(long)]
    pub(crate) json: bool,
}

impl PackValidateArgs {
    #[must_use]
    fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackImportArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Read imported workbench pack JSON from this path, or `-` for stdin.
    #[arg(default_value = "-")]
    pub(crate) input: String,
    /// Replace an existing pack with the same durable identifier.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackExportArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
    /// Write exported JSON to this path instead of stdout. Use `-` for stdout.
    #[arg(long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct PackDeleteArgs {
    #[command(flatten)]
    pub(crate) pack: PackIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewCommand {
    /// List durable operational review runs.
    List(ReviewListArgs),
    /// Show a durable review run.
    Show(ReviewShowArgs),
    /// Compare two compatible durable review runs.
    Diff(ReviewDiffArgs),
    /// Preview and apply safe remediation actions for review findings.
    Remediation(ReviewRemediationArgs),
    /// Update one durable review finding status.
    Mark(ReviewMarkArgs),
    /// Delete a durable review run by identifier.
    Delete(ReviewDeleteArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewListArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewShowArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDiffArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Baseline durable review run identifier.
    pub(crate) base_review_id: String,
    /// Target durable review run identifier.
    pub(crate) target_review_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationArgs {
    #[command(subcommand)]
    pub(crate) command: ReviewRemediationCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ReviewRemediationCommand {
    /// Inspect the daemon-owned remediation preview for one finding.
    Preview(ReviewRemediationPreviewArgs),
    /// Apply one supported remediation action after explicit confirmation.
    Apply(ReviewRemediationApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewFindingIdArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    pub(crate) finding_id: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationPreviewArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewRemediationApplyArgs {
    #[command(flatten)]
    pub(crate) finding: ReviewFindingIdArgs,
    /// Confirm applying the supported unlink-dangling-link remediation.
    #[arg(long)]
    pub(crate) confirm_unlink_dangling_link: bool,
    /// Replacement text for the removed id link. Defaults to the current link label.
    #[arg(long)]
    pub(crate) replacement_text: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewMarkArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// Durable review run identifier.
    pub(crate) review_id: String,
    /// Typed durable finding identifier within the review run.
    pub(crate) finding_id: String,
    /// New finding status: open, reviewed, dismissed, or accepted.
    pub(crate) status: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct ReviewDeleteArgs {
    #[command(flatten)]
    pub(crate) review: ReviewIdArgs,
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

pub(crate) fn run_sync(args: &SyncArgs) -> Result<(), CliCommandError> {
    match &args.command {
        SyncCommand::Root(command) => run_headless_command(command),
        SyncCommand::File(command) => run_headless_command(command),
    }
}

pub(crate) fn run_file(args: &FileArgs) -> Result<(), CliCommandError> {
    match &args.command {
        FileCommand::List(command) => run_headless_command(command),
        FileCommand::Search(command) => run_headless_command(command),
    }
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
enum MetadataField {
    Aliases,
    Refs,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataAction {
    Add,
    Remove,
    Set,
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
    let mut client = headless.connect()?;
    let node = resolve_note_target(&mut client, &target.target())
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let updated_values = metadata_values_for_action(&node, field, action, values);
    let updated = client
        .update_node_metadata(&metadata_update_params(
            node.node_key.clone(),
            field,
            updated_values,
        ))
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &updated, render_node_summary)
        .map_err(|error| CliCommandError::new(output_mode, error))
}

pub(crate) fn run_ref(args: &RefArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RefCommand::Search(command) => run_headless_command(command),
        RefCommand::Show(command) | RefCommand::Resolve(command) => run_headless_command(command),
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

pub(crate) fn run_daily(args: &DailyArgs) -> Result<(), CliCommandError> {
    match &args.command {
        DailyCommand::Ensure(command) => run_headless_command(command),
        DailyCommand::Show(command) => run_headless_command(command),
        DailyCommand::Append(command) => run_headless_command(command),
    }
}

pub(crate) fn run_graph(args: &GraphArgs) -> Result<(), CliCommandError> {
    match &args.command {
        GraphCommand::Dot(command) => run_graph_dot(command),
    }
}

pub(crate) fn run_capture(args: &CaptureArgs) -> Result<(), CliCommandError> {
    match &args.command {
        CaptureCommand::Node(command) => run_headless_command(command),
        CaptureCommand::Template(command) => run_capture_template(command),
        CaptureCommand::Preview(command) => run_capture_preview(command),
    }
}

pub(crate) fn run_note(args: &NoteArgs) -> Result<(), CliCommandError> {
    match &args.command {
        NoteCommand::Create(command) => run_headless_command(command),
        NoteCommand::EnsureFile(command) => run_headless_command(command),
        NoteCommand::AppendHeading(command) => run_headless_command(command),
        NoteCommand::AppendToNode(command) => run_headless_command(command),
        NoteCommand::AppendOutline(command) => run_headless_command(command),
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

pub(crate) fn run_link(args: &LinkArgs) -> Result<(), CliCommandError> {
    match &args.command {
        LinkCommand::RewriteSlipbox(command) => run_link_rewrite_slipbox(command),
    }
}

pub(crate) fn run_resolve_node(args: &ResolveNodeArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
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

pub(crate) fn run_audit(args: &AuditArgs) -> Result<(), CliCommandError> {
    match &args.command {
        AuditCommand::DanglingLinks(command) => {
            run_audit_command(CorpusAuditKind::DanglingLinks, command)
        }
        AuditCommand::DuplicateTitles(command) => {
            run_audit_command(CorpusAuditKind::DuplicateTitles, command)
        }
        AuditCommand::OrphanNotes(command) => {
            run_audit_command(CorpusAuditKind::OrphanNotes, command)
        }
        AuditCommand::WeaklyIntegratedNotes(command) => {
            run_audit_command(CorpusAuditKind::WeaklyIntegratedNotes, command)
        }
    }
}

pub(crate) fn run_workflow(args: &WorkflowArgs) -> Result<(), CliCommandError> {
    match &args.command {
        WorkflowCommand::List(command) => run_headless_command(command),
        WorkflowCommand::Show(command) => run_workflow_show(command),
        WorkflowCommand::Run(command) => run_workflow_command(command),
    }
}

pub(crate) fn run_routine(args: &RoutineArgs) -> Result<(), CliCommandError> {
    match &args.command {
        RoutineCommand::List(command) => run_headless_command(command),
        RoutineCommand::Show(command) => run_headless_command(command),
        RoutineCommand::Run(command) => run_headless_command(command),
    }
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

pub(crate) fn run_pack(args: &PackArgs) -> Result<(), CliCommandError> {
    match &args.command {
        PackCommand::List(command) => run_headless_command(command),
        PackCommand::Show(command) => run_headless_command(command),
        PackCommand::Validate(command) => run_pack_validate(command),
        PackCommand::Import(command) => run_pack_import(command),
        PackCommand::Export(command) => run_pack_export(command),
        PackCommand::Delete(command) => run_headless_command(command),
    }
}

pub(crate) fn run_review(args: &ReviewArgs) -> Result<(), CliCommandError> {
    match &args.command {
        ReviewCommand::List(command) => run_headless_command(command),
        ReviewCommand::Show(command) => run_headless_command(command),
        ReviewCommand::Diff(command) => run_headless_command(command),
        ReviewCommand::Remediation(command) => run_review_remediation(command),
        ReviewCommand::Mark(command) => run_review_mark(command),
        ReviewCommand::Delete(command) => run_headless_command(command),
    }
}

pub(crate) fn report_error(error: &CliCommandError) -> ExitCode {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    let _ = error.write(&mut writer);
    ExitCode::from(1)
}

#[derive(Debug, Serialize)]
struct ArtifactExportFileResult {
    artifact: ExplorationArtifactSummary,
    output_path: String,
}

#[derive(Debug, Serialize)]
struct PackExportFileResult {
    pack: WorkbenchPackSummary,
    output_path: String,
}

#[derive(Debug, Serialize)]
struct GraphDotFileResult {
    output_path: String,
    format: &'static str,
}

#[derive(Debug, Serialize)]
struct SavedExploreCommandResult {
    result: ExploreResult,
    artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Serialize)]
struct SavedCompareCommandResult {
    result: NoteComparisonResult,
    artifact: ExplorationArtifactSummary,
}

#[derive(Debug, Serialize)]
struct WorkflowReportOutputResult {
    workflow: WorkflowSummary,
    format: ReportFormat,
    output_path: String,
    step_count: usize,
}

#[derive(Debug, Serialize)]
struct SavedWorkflowReportOutputResult {
    workflow: WorkflowSummary,
    format: ReportFormat,
    output_path: String,
    step_count: usize,
    review: ReviewRunSummary,
}

#[derive(Debug, Serialize)]
struct AuditReportOutputResult {
    audit: CorpusAuditKind,
    format: ReportFormat,
    output_path: String,
    entry_count: usize,
}

#[derive(Debug, Serialize)]
struct SavedAuditReportOutputResult {
    audit: CorpusAuditKind,
    format: ReportFormat,
    output_path: String,
    entry_count: usize,
    review: ReviewRunSummary,
}

#[derive(Debug, Serialize)]
struct WorkflowShowFileResult {
    workflow: WorkflowSpec,
}

#[derive(Debug, Clone)]
struct SaveReviewRequest {
    review_id: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    overwrite: bool,
}

impl SaveReviewArgs {
    fn request(&self) -> Result<Option<SaveReviewRequest>> {
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
struct SaveArtifactRequest {
    metadata: ExplorationArtifactMetadata,
    overwrite: bool,
}

impl SaveArtifactArgs {
    fn request(&self) -> Result<Option<SaveArtifactRequest>> {
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

fn run_capture_template(command: &CaptureTemplateCommandArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let params = command
        .template
        .params()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let mut client = command.headless.connect()?;
    let captured = client
        .capture_template(&params)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &captured, |value| {
        render_anchor_summary(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_capture_preview(command: &CapturePreviewCommandArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let capture = command
        .template
        .params()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let source_override = command
        .source_override()
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let mut client = command.headless.connect()?;
    let preview = client
        .capture_template_preview(&CaptureTemplatePreviewParams {
            capture,
            source_override,
            ensure_node_id: command.ensure_node_id,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &preview, |value| {
        render_capture_preview(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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

fn run_pack_validate(command: &PackValidateArgs) -> Result<(), CliCommandError> {
    let output_mode = command.output_mode();
    let validation = validate_local_workbench_pack(&command.input)
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &validation, |value| {
        render_workbench_pack_validation(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_pack_import(command: &PackImportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let pack = read_workbench_pack_manifest(&command.input)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let mut client = command.headless.connect()?;
    let imported = client
        .import_workbench_pack(&ImportWorkbenchPackParams {
            pack,
            overwrite: command.overwrite,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &imported, |value| {
        format!(
            "imported pack: {} (workflows: {}, routines: {}, profiles: {})\n",
            value.pack.metadata.pack_id,
            value.pack.workflow_count,
            value.pack.review_routine_count,
            value.pack.report_profile_count
        )
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn run_pack_export(command: &PackExportArgs) -> Result<(), CliCommandError> {
    let output_mode = command.pack.headless.output_mode();
    let mut client = command.pack.headless.connect()?;
    let pack = client
        .export_workbench_pack(&WorkbenchPackIdParams {
            pack_id: command.pack.pack_id.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    if let Some(output_path) = &command.output
        && output_path != Path::new("-")
    {
        let serialized = serde_json::to_vec_pretty(&pack)
            .context("failed to serialize workbench pack")
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        fs::write(output_path, serialized)
            .with_context(|| {
                format!(
                    "failed to write exported workbench pack {}",
                    output_path.display()
                )
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;

        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let result = PackExportFileResult {
            pack: WorkbenchPackSummary::from(&pack),
            output_path: output_path.display().to_string(),
        };
        write_output(&mut writer, output_mode, &result, |value| {
            format!(
                "exported pack: {} -> {}\n",
                value.pack.metadata.pack_id, value.output_path
            )
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
        return Ok(());
    }

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    match output_mode {
        OutputMode::Human => {
            serde_json::to_writer_pretty(&mut writer, &pack)
                .context("failed to serialize workbench pack")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
            writer
                .write_all(b"\n")
                .map_err(|error| CliCommandError::new(output_mode, error))?;
        }
        OutputMode::Json => {
            serde_json::to_writer(&mut writer, &pack)
                .context("failed to serialize workbench pack")
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

fn run_workflow_show(command: &WorkflowShowArgs) -> Result<(), CliCommandError> {
    let output_mode = command.output_mode();
    let workflow = if let Some(spec_input) = &command.spec {
        let bytes = read_workflow_json_input(spec_input)
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        let compatibility: WorkflowSpecCompatibilityEnvelope = serde_json::from_slice(&bytes)
            .with_context(|| {
                if spec_input == "-" {
                    "failed to parse workflow spec JSON from stdin".to_owned()
                } else {
                    format!("failed to parse workflow spec JSON from {spec_input}")
                }
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        if let Some(message) = compatibility.compatibility.validation_error() {
            return Err(CliCommandError::new(
                output_mode,
                anyhow::anyhow!("invalid workflow spec: {message}"),
            ));
        }
        let workflow: WorkflowSpec = serde_json::from_slice(&bytes)
            .with_context(|| {
                if spec_input == "-" {
                    "failed to parse workflow spec JSON from stdin".to_owned()
                } else {
                    format!("failed to parse workflow spec JSON from {spec_input}")
                }
            })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        if let Some(message) = workflow.validation_error() {
            return Err(CliCommandError::new(
                output_mode,
                anyhow::anyhow!("invalid workflow spec: {message}"),
            ));
        }
        WorkflowShowFileResult { workflow }
    } else {
        let workflow_id = command
            .workflow_id
            .clone()
            .expect("clap enforces workflow source selection");
        let headless = command
            .headless_args()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        let mut client = headless.connect()?;
        let workflow = client
            .workflow(&WorkflowIdParams { workflow_id })
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(output_mode, error))?;
        WorkflowShowFileResult {
            workflow: workflow.workflow,
        }
    };

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &workflow, |value| {
        render_workflow_spec(&value.workflow)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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

fn read_pack_json_input(input: &str) -> Result<Vec<u8>> {
    if input == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read workbench pack JSON from stdin")?;
        return Ok(bytes);
    }

    fs::read(input).with_context(|| format!("failed to read workbench pack JSON from {input}"))
}

fn pack_json_parse_context(input: &str) -> String {
    if input == "-" {
        "failed to parse workbench pack JSON from stdin".to_owned()
    } else {
        format!("failed to parse workbench pack JSON from {input}")
    }
}

fn validate_local_workbench_pack(input: &str) -> Result<ValidateWorkbenchPackResult> {
    let bytes = read_pack_json_input(input)?;
    let compatibility: WorkbenchPackCompatibilityEnvelope =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    if let Some(message) = compatibility.compatibility.validation_error() {
        return Ok(ValidateWorkbenchPackResult {
            pack: None,
            valid: false,
            issues: vec![WorkbenchPackIssue {
                kind: WorkbenchPackIssueKind::UnsupportedVersion,
                asset_id: compatibility.pack_id,
                message,
            }],
        });
    }

    let pack: WorkbenchPackManifest =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    let issues = pack.validation_issues();
    Ok(ValidateWorkbenchPackResult {
        pack: issues.is_empty().then(|| WorkbenchPackSummary::from(&pack)),
        valid: issues.is_empty(),
        issues,
    })
}

fn read_workbench_pack_manifest(input: &str) -> Result<WorkbenchPackManifest> {
    let bytes = read_pack_json_input(input)?;
    let compatibility: WorkbenchPackCompatibilityEnvelope =
        serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))?;
    if let Some(message) = compatibility.compatibility.validation_error() {
        anyhow::bail!("invalid workbench pack: {message}");
    }
    serde_json::from_slice(&bytes).with_context(|| pack_json_parse_context(input))
}

fn read_workflow_json_input(input: &str) -> Result<Vec<u8>> {
    if input == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read workflow spec JSON from stdin")?;
        return Ok(bytes);
    }

    fs::read(input).with_context(|| format!("failed to read workflow spec JSON from {input}"))
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

fn render_report_bytes<T, L>(
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

fn write_report_destination(bytes: &[u8], output_path: Option<&Path>) -> Result<()> {
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

impl HeadlessCommand for SyncRootArgs {
    type Output = IndexStats;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.index()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        let mut rendered = String::from("refreshed root\n");
        rendered.push_str(&render_index_stats(output));
        rendered
    }
}

impl HeadlessCommand for SyncFileArgs {
    type Output = IndexFileResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.index_file(&IndexFileParams {
            file_path: self.path.display().to_string(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("refreshed file: {}\n", output.file_path)
    }
}

impl HeadlessCommand for FileListArgs {
    type Output = IndexedFilesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.indexed_files()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_indexed_files(output)
    }
}

impl HeadlessCommand for FileSearchArgs {
    type Output = SearchFilesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.search_files(&SearchFilesParams {
            query: self.query.clone(),
            limit: self.limit,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_file_search_result(output)
    }
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

impl HeadlessCommand for CaptureNodeCommandArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.capture_node(&CaptureNodeParams {
            title: self.title.clone(),
            file_path: self.file.as_ref().map(|path| path.display().to_string()),
            head: self.head.clone(),
            refs: self.refs.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NoteCreateArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.capture_node(&CaptureNodeParams {
            title: self.title.clone(),
            file_path: self.file.as_ref().map(|path| path.display().to_string()),
            head: self.head.clone(),
            refs: self.refs.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NoteEnsureFileArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.ensure_file_node(&EnsureFileNodeParams {
            file_path: self.file.display().to_string(),
            title: self.title.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for NoteAppendHeadingArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.append_heading(&AppendHeadingParams {
            file_path: self.file.display().to_string(),
            title: self.title.clone(),
            heading: self.heading.clone(),
            level: self.level,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NoteAppendToNodeArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let node = resolve_note_target(client, &self.target.target())?;
        client.append_heading_to_node(&AppendHeadingToNodeParams {
            node_key: node.node_key,
            heading: self.heading.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
    }
}

impl HeadlessCommand for NoteAppendOutlineArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.append_heading_at_outline_path(&AppendHeadingAtOutlinePathParams {
            file_path: self.file.display().to_string(),
            heading: self.heading.clone(),
            outline_path: self.outline_path.clone(),
            head: self.head.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
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

impl HeadlessCommand for RefShowArgs {
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

impl HeadlessCommand for DailyEnsureArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        ensure_daily_node(client, &target, self.head.as_deref())
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for DailyShowArgs {
    type Output = NodeRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: target.node_key(),
            })?,
            format!(
                "unknown daily note for {}: {}",
                target.date.format("%Y-%m-%d"),
                target.file_path
            ),
        )
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_node_summary(output)
    }
}

impl HeadlessCommand for DailyAppendArgs {
    type Output = AnchorRecord;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        let target = self.target.target()?;
        if self.head.is_some() {
            ensure_daily_node(client, &target, self.head.as_deref())?;
        }
        client.append_heading(&AppendHeadingParams {
            file_path: target.file_path,
            title: target.title,
            heading: self.heading.clone(),
            level: self.level,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_anchor_summary(output)
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

impl HeadlessCommand for WorkflowListArgs {
    type Output = ListWorkflowsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_workflows()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workflow_list(output)
    }
}

impl HeadlessCommand for WorkflowRunArgs {
    type Output = RunWorkflowResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.run_workflow(&RunWorkflowParams {
            workflow_id: self.workflow_id.clone(),
            inputs: parse_workflow_input_assignments(&self.inputs)?,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workflow_execution_result(&output.result)
    }
}

impl HeadlessCommand for RoutineListArgs {
    type Output = ListReviewRoutinesResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_review_routines()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_list(output)
    }
}

impl HeadlessCommand for RoutineShowArgs {
    type Output = ReviewRoutineResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.routine.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_routine(&ReviewRoutineIdParams {
            routine_id: self.routine.routine_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_spec(&output.routine)
    }
}

impl HeadlessCommand for RoutineRunArgs {
    type Output = RunReviewRoutineResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.run_review_routine(&RunReviewRoutineParams {
            routine_id: self.routine_id.clone(),
            inputs: parse_workflow_input_assignments(&self.inputs)?,
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_routine_execution_result(&output.result)
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

impl HeadlessCommand for PackListArgs {
    type Output = ListWorkbenchPacksResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_workbench_packs()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workbench_pack_list(output)
    }
}

impl HeadlessCommand for PackShowArgs {
    type Output = WorkbenchPackResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.pack.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.workbench_pack(&WorkbenchPackIdParams {
            pack_id: self.pack.pack_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_workbench_pack_manifest(&output.pack)
    }
}

impl HeadlessCommand for PackDeleteArgs {
    type Output = DeleteWorkbenchPackResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.pack.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_workbench_pack(&WorkbenchPackIdParams {
            pack_id: self.pack.pack_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted pack: {}\n", output.pack_id)
    }
}

impl HeadlessCommand for ReviewListArgs {
    type Output = ListReviewRunsResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.list_review_runs()
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_list(output)
    }
}

impl HeadlessCommand for ReviewShowArgs {
    type Output = ReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_run(&output.review)
    }
}

impl HeadlessCommand for ReviewDiffArgs {
    type Output = ReviewRunDiffResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.diff_review_runs(&ReviewRunDiffParams {
            base_review_id: self.base_review_id.clone(),
            target_review_id: self.target_review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_diff(&output.diff)
    }
}

impl HeadlessCommand for ReviewRemediationPreviewArgs {
    type Output = ReviewFindingRemediationPreviewResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.finding.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: self.finding.review_id.clone(),
            finding_id: self.finding.finding_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        render_review_remediation_preview(&output.preview)
    }
}

impl HeadlessCommand for ReviewDeleteArgs {
    type Output = DeleteReviewRunResult;

    fn headless_args(&self) -> &HeadlessArgs {
        &self.review.headless
    }

    fn execute(&self, client: &mut DaemonClient) -> Result<Self::Output, DaemonClientError> {
        client.delete_review_run(&ReviewRunIdParams {
            review_id: self.review.review_id.clone(),
        })
    }

    fn render_human(&self, output: &Self::Output) -> String {
        format!("deleted review: {}\n", output.review_id)
    }
}

fn run_review_remediation(command: &ReviewRemediationArgs) -> Result<(), CliCommandError> {
    match &command.command {
        ReviewRemediationCommand::Preview(command) => run_headless_command(command),
        ReviewRemediationCommand::Apply(command) => run_review_remediation_apply(command),
    }
}

fn run_review_remediation_apply(
    command: &ReviewRemediationApplyArgs,
) -> Result<(), CliCommandError> {
    let output_mode = command.finding.headless.output_mode();
    if !command.confirm_unlink_dangling_link {
        return Err(CliCommandError::new(
            output_mode,
            anyhow::anyhow!("review remediation apply requires --confirm-unlink-dangling-link"),
        ));
    }

    let mut client = command.finding.headless.connect()?;
    let preview = client
        .review_finding_remediation_preview(&ReviewFindingRemediationPreviewParams {
            review_id: command.finding.review_id.clone(),
            finding_id: command.finding.finding_id.clone(),
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?
        .preview;
    let action = remediation_action_from_preview(&preview, command.replacement_text.as_deref())
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let output = client
        .review_finding_remediation_apply(&ReviewFindingRemediationApplyParams {
            review_id: command.finding.review_id.clone(),
            finding_id: command.finding.finding_id.clone(),
            expected_preview: preview.preview_identity,
            action,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_review_remediation_application(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
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

fn remediation_action_from_preview(
    preview: &ReviewFindingRemediationPreview,
    replacement_text: Option<&str>,
) -> Result<AuditRemediationApplyAction> {
    match &preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview,
            ..
        } => {
            let replacement_text = match replacement_text {
                Some(value) => value.to_owned(),
                None => {
                    replacement_text_from_dangling_preview(preview, *column, missing_explicit_id)?
                }
            };
            Ok(AuditRemediationApplyAction::UnlinkDanglingLink {
                source_node_key: source.node_key.clone(),
                missing_explicit_id: missing_explicit_id.clone(),
                file_path: file_path.clone(),
                line: *line,
                column: *column,
                preview: preview.clone(),
                replacement_text,
            })
        }
        AuditRemediationPreviewPayload::DuplicateTitle { .. } => {
            anyhow::bail!(
                "review remediation apply currently supports only unlink-dangling-link findings"
            )
        }
    }
}

fn replacement_text_from_dangling_preview(
    preview: &str,
    column: u32,
    missing_explicit_id: &str,
) -> Result<String> {
    let link = org_link_at_preview_column(preview, column)
        .or_else(|| first_org_id_link_for_target(preview, missing_explicit_id))
        .ok_or_else(|| {
            anyhow::anyhow!("failed to derive unlink-dangling-link replacement text from preview")
        })?;
    let (target, label) = org_id_link_target_and_label(link).ok_or_else(|| {
        anyhow::anyhow!("failed to derive unlink-dangling-link replacement text from preview")
    })?;
    if target != missing_explicit_id {
        anyhow::bail!(
            "preview link target {target} does not match missing id {missing_explicit_id}"
        );
    }
    Ok(label.unwrap_or(target).to_owned())
}

fn org_link_at_preview_column(preview: &str, column: u32) -> Option<&str> {
    let link_start = byte_index_for_column(preview, column)?;
    let suffix = preview.get(link_start..)?;
    if !suffix.starts_with("[[") {
        return None;
    }
    let link_end = suffix.find("]]")? + 2;
    suffix.get(..link_end)
}

fn first_org_id_link_for_target<'a>(preview: &'a str, target: &str) -> Option<&'a str> {
    let mut search_start = 0_usize;
    while let Some(relative_start) = preview.get(search_start..)?.find("[[") {
        let start = search_start + relative_start;
        let suffix = preview.get(start..)?;
        let link_end = suffix.find("]]")? + 2;
        let link = suffix.get(..link_end)?;
        if org_id_link_target_and_label(link).is_some_and(|(candidate, _)| candidate == target) {
            return Some(link);
        }
        search_start = start + link_end;
    }
    None
}

fn byte_index_for_column(line: &str, column: u32) -> Option<usize> {
    if column == 0 {
        return None;
    }
    if column == 1 {
        return Some(0);
    }
    line.char_indices()
        .nth(column as usize - 1)
        .map(|(index, _)| index)
}

fn org_id_link_target_and_label(link: &str) -> Option<(&str, Option<&str>)> {
    let inner = link.strip_prefix("[[")?.strip_suffix("]]")?;
    let (target, label) = inner
        .split_once("][")
        .map_or((inner, None), |(target, label)| (target, Some(label)));
    target
        .trim()
        .strip_prefix("id:")
        .map(|id| (id.trim(), label))
}

fn run_review_mark(command: &ReviewMarkArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let status = parse_review_finding_status(&command.status)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let mut client = command.headless.connect()?;
    let output = client
        .mark_review_finding(&MarkReviewFindingParams {
            review_id: command.review_id.clone(),
            finding_id: command.finding_id.clone(),
            status,
        })
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(output_mode, error))?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_output(&mut writer, output_mode, &output, |value| {
        render_mark_review_finding_result(value)
    })
    .map_err(|error| CliCommandError::new(output_mode, error))
}

fn parse_review_finding_status(value: &str) -> Result<ReviewFindingStatus> {
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

fn parse_workflow_input_assignments(
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

fn resolve_anchor_or_note_target_key(
    client: &mut DaemonClient,
    target: &ResolveTarget,
) -> Result<String, DaemonClientError> {
    match target {
        ResolveTarget::Key(node_key) => Ok(node_key.clone()),
        _ => resolve_note_target(client, target).map(|node| node.node_key),
    }
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

fn require_resolved_node(
    node: Option<NodeRecord>,
    error_message: String,
) -> Result<NodeRecord, DaemonClientError> {
    node.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

fn require_resolved_anchor(
    anchor: Option<AnchorRecord>,
    error_message: String,
) -> Result<AnchorRecord, DaemonClientError> {
    anchor.ok_or_else(|| DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(error_message)))
}

fn invalid_request_error(message: impl Into<String>) -> DaemonClientError {
    DaemonClientError::Rpc(JsonRpcErrorObject::invalid_request(message.into()))
}

fn parse_agenda_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    parse_iso_date(value, "agenda")
}

fn parse_daily_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    parse_iso_date(value, "daily")
}

fn parse_iso_date(value: &str, label: &str) -> Result<NaiveDate, DaemonClientError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid_request_error(format!(
            "invalid {label} date {value:?}: expected ISO date YYYY-MM-DD"
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

fn ensure_daily_node(
    client: &mut DaemonClient,
    target: &DailyTarget,
    head: Option<&str>,
) -> Result<NodeRecord, DaemonClientError> {
    if let Some(head) = head {
        if let Some(existing) = client.node_from_key(&NodeFromKeyParams {
            node_key: target.node_key(),
        })? {
            return Ok(existing);
        }

        client.capture_template(&CaptureTemplateParams {
            title: target.title.clone(),
            file_path: Some(target.file_path.clone()),
            node_key: None,
            head: Some(target.date.format(head).to_string()),
            outline_path: Vec::new(),
            capture_type: CaptureContentType::Plain,
            content: String::new(),
            refs: Vec::new(),
            prepend: false,
            empty_lines_before: 0,
            empty_lines_after: 0,
            table_line_pos: None,
        })?;
        return require_resolved_node(
            client.node_from_key(&NodeFromKeyParams {
                node_key: target.node_key(),
            })?,
            format!(
                "unknown daily note for {} after ensure: {}",
                target.date.format("%Y-%m-%d"),
                target.file_path
            ),
        );
    }

    client.ensure_file_node(&EnsureFileNodeParams {
        file_path: target.file_path.clone(),
        title: target.title.clone(),
    })
}

fn run_audit_command(kind: CorpusAuditKind, args: &AuditRunArgs) -> Result<(), CliCommandError> {
    let output_mode = args.headless.output_mode();
    let report_format = args
        .report
        .format(output_mode)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let error_output_mode = report_format.error_output_mode();
    let save_review = args
        .save_review
        .request()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    let mut client = args.headless.connect_with_output_mode(error_output_mode)?;

    if let Some(save_review) = save_review {
        let saved = client
            .save_corpus_audit_review(&SaveCorpusAuditReviewParams {
                audit: kind,
                limit: args.limit,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        return write_saved_audit_command_output(args, report_format, saved, error_output_mode);
    }

    let result = client
        .corpus_audit(&CorpusAuditParams {
            audit: kind,
            limit: args.limit,
        })
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    let report_bytes = render_report_bytes(
        report_format,
        &result,
        render_corpus_audit_result,
        CorpusAuditResult::report_lines,
    )
    .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    write_report_destination(&report_bytes, args.report.output_path())
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    if let Some(output_path) = args.report.output_path() {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = AuditReportOutputResult {
            audit: result.audit,
            format: report_format,
            output_path: output_path.display().to_string(),
            entry_count: result.entries.len(),
        };
        write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                format!(
                    "wrote audit report: {} -> {} ({})\n",
                    render_corpus_audit_kind(value.audit),
                    value.output_path,
                    value.format.label(),
                )
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error))?;
    }
    Ok(())
}

fn run_workflow_command(command: &WorkflowRunArgs) -> Result<(), CliCommandError> {
    let output_mode = command.headless.output_mode();
    let report_format = command
        .report
        .format(output_mode)
        .map_err(|error| CliCommandError::new(output_mode, error))?;
    let error_output_mode = report_format.error_output_mode();
    let save_review = command
        .save_review
        .request()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    let mut client = command
        .headless
        .connect_with_output_mode(error_output_mode)?;

    if let Some(save_review) = save_review {
        let saved = client
            .save_workflow_review(&SaveWorkflowReviewParams {
                workflow_id: command.workflow_id.clone(),
                inputs: parse_workflow_input_assignments(&command.inputs)
                    .map_err(|error| CliCommandError::new(error_output_mode, error))?,
                review_id: save_review.review_id,
                title: save_review.title,
                summary: save_review.summary,
                overwrite: save_review.overwrite,
            })
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        client
            .shutdown()
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        return write_saved_workflow_command_output(
            command,
            report_format,
            saved,
            error_output_mode,
        );
    }

    let result = command
        .execute(&mut client)
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    client
        .shutdown()
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    let report_bytes = render_report_bytes(
        report_format,
        &result,
        |value| command.render_human(value),
        |value| value.result.report_lines(),
    )
    .map_err(|error| CliCommandError::new(error_output_mode, error))?;
    write_report_destination(&report_bytes, command.report.output_path())
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;

    if let Some(output_path) = command.report.output_path() {
        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = WorkflowReportOutputResult {
            workflow: result.result.workflow.clone(),
            format: report_format,
            output_path: output_path.display().to_string(),
            step_count: result.result.steps.len(),
        };
        write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                format!(
                    "wrote workflow report: {} -> {} ({})\n",
                    value.workflow.metadata.workflow_id,
                    value.output_path,
                    value.format.label(),
                )
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error))?;
    }
    Ok(())
}

fn write_saved_audit_command_output(
    args: &AuditRunArgs,
    report_format: ReportFormat,
    saved: SaveCorpusAuditReviewResult,
    error_output_mode: OutputMode,
) -> Result<(), CliCommandError> {
    if let Some(output_path) = args.report.output_path() {
        let report_bytes = render_report_bytes(
            report_format,
            &saved.result,
            render_corpus_audit_result,
            CorpusAuditResult::report_lines,
        )
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        write_report_destination(&report_bytes, Some(output_path))
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;

        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = SavedAuditReportOutputResult {
            audit: saved.result.audit,
            format: report_format,
            output_path: output_path.display().to_string(),
            entry_count: saved.result.entries.len(),
            review: saved.review,
        };
        return write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                let mut output = format!(
                    "wrote audit report: {} -> {} ({})\n",
                    render_corpus_audit_kind(value.audit),
                    value.output_path,
                    value.format.label(),
                );
                output.push_str(&render_saved_review_summary(&value.review));
                output
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error));
    }

    match report_format {
        ReportFormat::Human => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Human, &saved, |value| {
                let mut output = render_corpus_audit_result(&value.result);
                output.push('\n');
                output.push_str(&render_saved_review_summary(&value.review));
                output
            })
            .map_err(|error| CliCommandError::new(OutputMode::Human, error))
        }
        ReportFormat::Json => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Json, &saved, |_| String::new())
                .map_err(|error| CliCommandError::new(OutputMode::Json, error))
        }
        ReportFormat::Jsonl => {
            let report_bytes = render_report_bytes(
                report_format,
                &saved.result,
                render_corpus_audit_result,
                CorpusAuditResult::report_lines,
            )
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
            write_report_destination(&report_bytes, None)
                .map_err(|error| CliCommandError::new(error_output_mode, error))
        }
    }
}

fn write_saved_workflow_command_output(
    command: &WorkflowRunArgs,
    report_format: ReportFormat,
    saved: SaveWorkflowReviewResult,
    error_output_mode: OutputMode,
) -> Result<(), CliCommandError> {
    if let Some(output_path) = command.report.output_path() {
        let report_bytes = render_report_bytes(
            report_format,
            &saved.result,
            render_workflow_execution_result,
            WorkflowExecutionResult::report_lines,
        )
        .map_err(|error| CliCommandError::new(error_output_mode, error))?;
        write_report_destination(&report_bytes, Some(output_path))
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;

        let stdout = io::stdout();
        let mut writer = stdout.lock();
        let ack = SavedWorkflowReportOutputResult {
            workflow: saved.result.workflow.clone(),
            format: report_format,
            output_path: output_path.display().to_string(),
            step_count: saved.result.steps.len(),
            review: saved.review,
        };
        return write_output(
            &mut writer,
            report_format.ack_output_mode(),
            &ack,
            |value| {
                let mut output = format!(
                    "wrote workflow report: {} -> {} ({})\n",
                    value.workflow.metadata.workflow_id,
                    value.output_path,
                    value.format.label(),
                );
                output.push_str(&render_saved_review_summary(&value.review));
                output
            },
        )
        .map_err(|error| CliCommandError::new(report_format.ack_output_mode(), error));
    }

    match report_format {
        ReportFormat::Human => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Human, &saved, |value| {
                let mut output = render_workflow_execution_result(&value.result);
                output.push('\n');
                output.push_str(&render_saved_review_summary(&value.review));
                output
            })
            .map_err(|error| CliCommandError::new(OutputMode::Human, error))
        }
        ReportFormat::Json => {
            let stdout = io::stdout();
            let mut writer = stdout.lock();
            write_output(&mut writer, OutputMode::Json, &saved, |_| String::new())
                .map_err(|error| CliCommandError::new(OutputMode::Json, error))
        }
        ReportFormat::Jsonl => {
            let report_bytes = render_report_bytes(
                report_format,
                &saved.result,
                render_workflow_execution_result,
                WorkflowExecutionResult::report_lines,
            )
            .map_err(|error| CliCommandError::new(error_output_mode, error))?;
            write_report_destination(&report_bytes, None)
                .map_err(|error| CliCommandError::new(error_output_mode, error))
        }
    }
}

fn render_flag_list(flags: &[&str]) -> String {
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

fn render_workflow_list(result: &ListWorkflowsResult) -> String {
    let mut output = String::new();
    if result.workflows.is_empty() {
        output.push_str("(none)\n");
    } else {
        for workflow in &result.workflows {
            output.push_str(&format!(
                "- {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("  steps: {}\n", workflow.step_count));
            if let Some(summary) = &workflow.metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

fn render_workflow_catalog_issues(output: &mut String, issues: &[WorkflowCatalogIssue]) {
    output.push_str("[issues]\n");
    for issue in issues {
        output.push_str(&format!("- path: {}\n", issue.path));
        output.push_str(&format!("  kind: {}\n", issue.kind.label()));
        if let Some(pack_id) = &issue.pack_id {
            output.push_str(&format!("  pack id: {pack_id}\n"));
        }
        if let Some(workflow_id) = &issue.workflow_id {
            output.push_str(&format!("  workflow id: {workflow_id}\n"));
        }
        if let Some(routine_id) = &issue.routine_id {
            output.push_str(&format!("  routine id: {routine_id}\n"));
        }
        if let Some(profile_id) = &issue.profile_id {
            output.push_str(&format!("  profile id: {profile_id}\n"));
        }
        output.push_str(&format!("  message: {}\n", issue.message));
    }
}

fn render_workbench_pack_list(result: &ListWorkbenchPacksResult) -> String {
    let mut output = String::new();
    if result.packs.is_empty() {
        output.push_str("(none)\n");
    } else {
        for pack in &result.packs {
            output.push_str(&format!(
                "- {} [{}]\n",
                pack.metadata.title, pack.metadata.pack_id
            ));
            output.push_str(&format!(
                "  workflows/routines/profiles: {}/{}/{}\n",
                pack.workflow_count, pack.review_routine_count, pack.report_profile_count
            ));
            output.push_str(&format!(
                "  compatibility: workbench-pack/v{}\n",
                pack.compatibility.version
            ));
            if !pack.entrypoint_routine_ids.is_empty() {
                output.push_str(&format!(
                    "  entrypoint routines: {}\n",
                    pack.entrypoint_routine_ids.join(", ")
                ));
            }
            if let Some(summary) = &pack.metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

fn render_workbench_pack_manifest(pack: &WorkbenchPackManifest) -> String {
    let mut output = String::new();
    output.push_str(&format!("pack id: {}\n", pack.metadata.pack_id));
    output.push_str(&format!("title: {}\n", pack.metadata.title));
    output.push_str(&format!(
        "compatibility: workbench-pack/v{}\n",
        pack.compatibility.version
    ));
    if let Some(summary) = &pack.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    output.push_str(&format!("workflows: {}\n", pack.workflows.len()));
    output.push_str(&format!(
        "review routines: {}\n",
        pack.review_routines.len()
    ));
    output.push_str(&format!(
        "report profiles: {}\n",
        pack.report_profiles.len()
    ));
    if !pack.entrypoint_routine_ids.is_empty() {
        output.push_str(&format!(
            "entrypoint routines: {}\n",
            pack.entrypoint_routine_ids.join(", ")
        ));
    }

    if !pack.workflows.is_empty() {
        output.push_str("\n[workflows]\n");
        for workflow in &pack.workflows {
            output.push_str(&format!(
                "- {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("  steps: {}\n", workflow.steps.len()));
        }
    }
    if !pack.review_routines.is_empty() {
        output.push_str("\n[review-routines]\n");
        for routine in &pack.review_routines {
            output.push_str(&format!(
                "- {} [{}]\n",
                routine.metadata.title, routine.metadata.routine_id
            ));
            output.push_str(&format!("  source: {}\n", routine.source.kind().label()));
        }
    }
    if !pack.report_profiles.is_empty() {
        output.push_str("\n[report-profiles]\n");
        for profile in &pack.report_profiles {
            output.push_str(&format!(
                "- {} [{}]\n",
                profile.metadata.title, profile.metadata.profile_id
            ));
        }
    }

    output
}

fn render_workbench_pack_validation(result: &ValidateWorkbenchPackResult) -> String {
    let mut output = String::new();
    if result.valid {
        if let Some(pack) = &result.pack {
            output.push_str(&format!(
                "valid pack: {} (workflows: {}, routines: {}, profiles: {})\n",
                pack.metadata.pack_id,
                pack.workflow_count,
                pack.review_routine_count,
                pack.report_profile_count
            ));
        } else {
            output.push_str("valid pack\n");
        }
        return output;
    }

    output.push_str("invalid pack\n");
    render_workbench_pack_issues(&mut output, &result.issues);
    output
}

fn render_workbench_pack_issues(output: &mut String, issues: &[WorkbenchPackIssue]) {
    if issues.is_empty() {
        return;
    }
    output.push_str("[issues]\n");
    for issue in issues {
        output.push_str(&format!("- kind: {}\n", issue.kind.label()));
        if let Some(asset_id) = &issue.asset_id {
            output.push_str(&format!("  asset id: {asset_id}\n"));
        }
        output.push_str(&format!("  message: {}\n", issue.message));
    }
}

fn render_review_routine_list(result: &ListReviewRoutinesResult) -> String {
    let mut output = String::new();
    if result.routines.is_empty() {
        output.push_str("(none)\n");
    } else {
        for routine in &result.routines {
            render_review_routine_summary(&mut output, routine);
        }
    }

    if !result.issues.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        render_workflow_catalog_issues(&mut output, &result.issues);
    }

    output
}

fn render_review_routine_summary(output: &mut String, routine: &ReviewRoutineSummary) {
    output.push_str(&format!(
        "- {} [{}]\n",
        routine.metadata.title, routine.metadata.routine_id
    ));
    output.push_str(&format!("  source: {}\n", routine.source_kind.label()));
    output.push_str(&format!("  inputs: {}\n", routine.input_count));
    output.push_str(&format!(
        "  report profiles: {}\n",
        routine.report_profile_count
    ));
    if let Some(summary) = &routine.metadata.summary {
        output.push_str(&format!("  summary: {summary}\n"));
    }
}

fn render_review_routine_spec(routine: &ReviewRoutineSpec) -> String {
    let mut output = String::new();
    output.push_str(&format!("routine id: {}\n", routine.metadata.routine_id));
    output.push_str(&format!("title: {}\n", routine.metadata.title));
    output.push_str(&format!("source: {}\n", routine.source.kind().label()));
    if let Some(summary) = &routine.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    render_review_routine_source(&mut output, &routine.source);
    output.push_str(&format!("save review: {}\n", routine.save_review.enabled));
    if let Some(review_id) = &routine.save_review.review_id {
        output.push_str(&format!("review id: {review_id}\n"));
    }
    if let Some(title) = &routine.save_review.title {
        output.push_str(&format!("review title: {title}\n"));
    }
    if let Some(summary) = &routine.save_review.summary {
        output.push_str(&format!("review summary: {summary}\n"));
    }
    output.push_str(&format!("overwrite: {}\n", routine.save_review.overwrite));
    if let Some(compare) = &routine.compare {
        output.push_str(&format!("compare: {}\n", compare.target.label()));
        if let Some(profile_id) = &compare.report_profile_id {
            output.push_str(&format!("compare report profile: {profile_id}\n"));
        }
    }
    if !routine.report_profile_ids.is_empty() {
        output.push_str(&format!(
            "report profiles: {}\n",
            routine.report_profile_ids.join(", ")
        ));
    }
    if !routine.inputs.is_empty() {
        output.push_str("\n[inputs]\n");
        for input in &routine.inputs {
            render_workflow_input_spec(&mut output, input);
        }
    }
    output
}

fn render_review_routine_source(output: &mut String, source: &ReviewRoutineSource) {
    match source {
        ReviewRoutineSource::Audit { audit, limit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
            output.push_str(&format!("limit: {limit}\n"));
        }
        ReviewRoutineSource::Workflow { workflow_id } => {
            output.push_str(&format!("workflow id: {workflow_id}\n"));
        }
        ReviewRoutineSource::Unsupported => {
            output.push_str("unsupported source\n");
        }
    }
}

fn render_review_routine_execution_result(result: &ReviewRoutineExecutionResult) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "routine: {} [{}]\n",
        result.routine.metadata.title, result.routine.metadata.routine_id
    ));
    output.push_str(&format!("source: {}\n", result.routine.source_kind.label()));
    if let Some(summary) = &result.routine.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    match &result.source {
        ReviewRoutineSourceExecutionResult::Audit { result } => {
            output.push('\n');
            output.push_str(&render_corpus_audit_result(result));
        }
        ReviewRoutineSourceExecutionResult::Workflow { result } => {
            output.push('\n');
            output.push_str(&render_workflow_execution_result(result));
        }
    }
    if let Some(saved_review) = &result.saved_review {
        output.push('\n');
        output.push_str(&render_saved_review_summary(saved_review));
        output.push_str(&format!("{}\n", render_review_status_counts(saved_review)));
    }
    if let Some(compare) = &result.compare {
        output.push('\n');
        output.push_str(&render_review_routine_compare(compare));
    }
    if !result.reports.is_empty() {
        output.push('\n');
        output.push_str("[reports]\n");
        for report in &result.reports {
            render_applied_report_profile(&mut output, report);
        }
    }
    output
}

fn render_review_routine_compare(compare: &ReviewRoutineCompareResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("compare: {}\n", compare.target.label()));
    if let Some(base_review) = &compare.base_review {
        output.push_str(&format!(
            "base review: {} [{}]\n",
            base_review.metadata.title, base_review.metadata.review_id
        ));
    } else {
        output.push_str("base review: none\n");
    }
    if let Some(diff) = &compare.diff {
        output.push_str(&render_review_diff(diff));
    }
    if let Some(report) = &compare.report {
        output.push_str("\n[compare-report]\n");
        render_applied_report_profile(&mut output, report);
    }
    output
}

fn render_applied_report_profile(output: &mut String, report: &AppliedReportProfile) {
    output.push_str(&format!(
        "- {} [{}]\n",
        report.profile.metadata.title, report.profile.metadata.profile_id
    ));
    if report.lines.is_empty() {
        output.push_str("  (no lines)\n");
        return;
    }
    for line in &report.lines {
        let rendered = render_review_routine_report_line(line);
        push_indented(output, &rendered, "  ");
    }
}

fn render_review_routine_report_line(line: &ReviewRoutineReportLine) -> String {
    let mut output = String::new();
    match line {
        ReviewRoutineReportLine::Routine { routine } => {
            output.push_str(&format!(
                "routine: {} [{}]\n",
                routine.metadata.title, routine.metadata.routine_id
            ));
        }
        ReviewRoutineReportLine::Workflow { workflow } => {
            output.push_str(&format!(
                "workflow: {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
        }
        ReviewRoutineReportLine::Step { step } => {
            render_workflow_step_report(&mut output, step);
        }
        ReviewRoutineReportLine::Audit { audit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
        }
        ReviewRoutineReportLine::Entry { entry } => {
            render_review_audit_entry(&mut output, entry);
        }
        ReviewRoutineReportLine::Review { review } => {
            output.push_str(&render_saved_review_summary(review));
            output.push_str(&format!("{}\n", render_review_status_counts(review)));
        }
        ReviewRoutineReportLine::Finding { finding }
        | ReviewRoutineReportLine::Added { finding }
        | ReviewRoutineReportLine::Removed { finding } => {
            render_review_finding(&mut output, finding, "");
        }
        ReviewRoutineReportLine::Diff {
            base_review,
            target_review,
        } => {
            output.push_str(&format!(
                "diff: {} -> {}\n",
                base_review.metadata.review_id, target_review.metadata.review_id
            ));
        }
        ReviewRoutineReportLine::Unchanged { finding }
        | ReviewRoutineReportLine::ContentChanged { finding } => {
            output.push_str(&format!("finding pair: {}\n", finding.finding_id));
            output.push_str("base:\n");
            render_review_finding(&mut output, &finding.base, "  ");
            output.push_str("target:\n");
            render_review_finding(&mut output, &finding.target, "  ");
        }
        ReviewRoutineReportLine::StatusChanged { change } => {
            output.push_str(&format!("status changed: {}\n", change.finding_id));
            output.push_str(&format!(
                "status: {} -> {}\n",
                render_review_finding_status(change.from_status),
                render_review_finding_status(change.to_status)
            ));
            render_review_finding(&mut output, &change.target, "");
        }
    }
    output
}

fn render_workflow_spec(workflow: &WorkflowSpec) -> String {
    let mut output = String::new();
    output.push_str(&format!("workflow id: {}\n", workflow.metadata.workflow_id));
    output.push_str(&format!("title: {}\n", workflow.metadata.title));
    output.push_str(&format!(
        "compatibility: workflow-spec/v{}\n",
        workflow.compatibility.version
    ));
    if let Some(summary) = &workflow.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    output.push_str(&format!("steps: {}\n", workflow.steps.len()));
    if !workflow.inputs.is_empty() {
        output.push_str("\n[inputs]\n");
        for input in &workflow.inputs {
            render_workflow_input_spec(&mut output, input);
        }
    }
    output.push_str("\n[steps]\n");
    for step in &workflow.steps {
        render_workflow_step_spec(&mut output, step);
    }
    output
}

fn render_corpus_audit_result(result: &CorpusAuditResult) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "audit: {}\n",
        render_corpus_audit_kind(result.audit)
    ));
    if result.entries.is_empty() {
        output.push_str("(none)\n");
        return output;
    }
    for entry in &result.entries {
        match entry {
            CorpusAuditEntry::DanglingLink { record } => {
                output.push_str(&format!(
                    "\n- {} -> missing id {}\n",
                    render_anchor_identity(&record.source),
                    record.missing_explicit_id
                ));
                output.push_str(&format!(
                    "  location: {}:{}:{}\n",
                    record.source.file_path, record.line, record.column
                ));
                output.push_str(&format!("  preview: {}\n", record.preview));
            }
            CorpusAuditEntry::DuplicateTitle { record } => {
                output.push_str(&format!("\n- duplicate title: {}\n", record.title));
                for note in &record.notes {
                    output.push_str(&format!("  note: {} [{}]\n", note.title, note.node_key));
                    output.push_str(&format!("  file: {}:{}\n", note.file_path, note.line));
                }
            }
            CorpusAuditEntry::OrphanNote { record } => {
                output.push_str(&format!(
                    "\n- orphan note: {} [{}]\n",
                    record.note.title, record.note.node_key
                ));
                output.push_str(&format!(
                    "  refs/backlinks/forward-links: {}/{}/{}\n",
                    record.reference_count, record.backlink_count, record.forward_link_count
                ));
            }
            CorpusAuditEntry::WeaklyIntegratedNote { record } => {
                output.push_str(&format!(
                    "\n- weakly integrated note: {} [{}]\n",
                    record.note.title, record.note.node_key
                ));
                output.push_str(&format!(
                    "  refs/backlinks/forward-links: {}/{}/{}\n",
                    record.reference_count, record.backlink_count, record.forward_link_count
                ));
            }
        }
    }
    output
}

fn render_corpus_audit_kind(kind: CorpusAuditKind) -> &'static str {
    match kind {
        CorpusAuditKind::DanglingLinks => "dangling-links",
        CorpusAuditKind::DuplicateTitles => "duplicate-titles",
        CorpusAuditKind::OrphanNotes => "orphan-notes",
        CorpusAuditKind::WeaklyIntegratedNotes => "weakly-integrated-notes",
    }
}

fn render_workflow_input_spec(output: &mut String, input: &WorkflowInputSpec) {
    output.push_str(&format!(
        "- {} [{}]\n",
        input.input_id,
        render_workflow_input_kind(input.kind)
    ));
    output.push_str(&format!("  title: {}\n", input.title));
    if let Some(summary) = &input.summary {
        output.push_str(&format!("  summary: {summary}\n"));
    }
}

fn render_workflow_step_spec(output: &mut String, step: &WorkflowStepSpec) {
    output.push_str(&format!("- {} [{}]\n", step.step_id, step.kind().label()));
    match &step.payload {
        WorkflowStepPayload::Resolve { target } => {
            output.push_str(&format!(
                "  target: {}\n",
                render_workflow_resolve_target(target)
            ));
        }
        WorkflowStepPayload::Explore {
            focus,
            lens,
            limit,
            unique,
        } => {
            output.push_str(&format!(
                "  focus: {}\n",
                render_workflow_explore_focus(focus)
            ));
            output.push_str(&format!("  lens: {}\n", render_exploration_lens(*lens)));
            output.push_str(&format!("  limit: {limit}\n"));
            output.push_str(&format!("  unique: {unique}\n"));
        }
        WorkflowStepPayload::Compare {
            left,
            right,
            group,
            limit,
        } => {
            output.push_str(&format!("  left: {}\n", left.step_id));
            output.push_str(&format!("  right: {}\n", right.step_id));
            output.push_str(&format!("  group: {}\n", render_comparison_group(*group)));
            output.push_str(&format!("  limit: {limit}\n"));
        }
        WorkflowStepPayload::ArtifactRun { artifact_id } => {
            output.push_str(&format!("  artifact id: {artifact_id}\n"));
        }
        WorkflowStepPayload::ArtifactSave {
            source,
            metadata,
            overwrite,
        } => {
            output.push_str(&format!(
                "  source: {}\n",
                render_workflow_artifact_save_source(source)
            ));
            output.push_str(&format!("  artifact id: {}\n", metadata.artifact_id));
            output.push_str(&format!("  title: {}\n", metadata.title));
            if let Some(summary) = &metadata.summary {
                output.push_str(&format!("  summary: {summary}\n"));
            }
            output.push_str(&format!("  overwrite: {overwrite}\n"));
        }
    }
}

fn render_workflow_execution_result(result: &WorkflowExecutionResult) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "workflow: {} [{}]\n",
        result.workflow.metadata.title, result.workflow.metadata.workflow_id
    ));
    output.push_str(&format!("steps: {}\n", result.workflow.step_count));
    if let Some(summary) = &result.workflow.metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
    for step in &result.steps {
        output.push('\n');
        render_workflow_step_report(&mut output, step);
    }
    output
}

fn render_workflow_step_report(output: &mut String, step: &WorkflowStepReport) {
    output.push_str(&format!("[step {}]\n", step.step_id));
    output.push_str(&format!("kind: {}\n", step.kind().label()));
    match &step.payload {
        WorkflowStepReportPayload::Resolve { node } => {
            output.push_str(&render_node_summary(node));
        }
        WorkflowStepReportPayload::Explore {
            focus_node_key,
            result,
        } => {
            output.push_str(&format!("focus node key: {focus_node_key}\n"));
            output.push_str(&render_explore_result(result));
        }
        WorkflowStepReportPayload::Compare {
            left_node,
            right_node,
            result,
        } => {
            output.push_str(&format!("left node: {}\n", render_node_identity(left_node)));
            output.push_str(&format!(
                "right node: {}\n",
                render_node_identity(right_node)
            ));
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
        WorkflowStepReportPayload::ArtifactRun { artifact } => {
            output.push_str(&render_executed_exploration_artifact(artifact));
        }
        WorkflowStepReportPayload::ArtifactSave { artifact } => {
            output.push_str(&render_saved_artifact_summary(artifact));
        }
    }
}

fn render_workflow_input_kind(kind: WorkflowInputKind) -> &'static str {
    match kind {
        WorkflowInputKind::NoteTarget => "note-target",
        WorkflowInputKind::FocusTarget => "focus-target",
    }
}

fn render_workflow_resolve_target(target: &WorkflowResolveTarget) -> String {
    match target {
        WorkflowResolveTarget::Id { id } => format!("id:{id}"),
        WorkflowResolveTarget::Title { title } => format!("title:{title}"),
        WorkflowResolveTarget::Reference { reference } => format!("ref:{reference}"),
        WorkflowResolveTarget::NodeKey { node_key } => format!("key:{node_key}"),
        WorkflowResolveTarget::Input { input_id } => format!("input:{input_id}"),
    }
}

fn render_workflow_explore_focus(focus: &WorkflowExploreFocus) -> String {
    match focus {
        WorkflowExploreFocus::NodeKey { node_key } => format!("key:{node_key}"),
        WorkflowExploreFocus::Input { input_id } => format!("input:{input_id}"),
        WorkflowExploreFocus::ResolvedStep { step_id } => format!("resolved-step:{step_id}"),
    }
}

fn render_workflow_artifact_save_source(source: &WorkflowArtifactSaveSource) -> String {
    match source {
        WorkflowArtifactSaveSource::ExploreStep { step_id } => {
            format!("explore-step:{step_id}")
        }
        WorkflowArtifactSaveSource::CompareStep { step_id } => {
            format!("compare-step:{step_id}")
        }
    }
}

fn render_saved_artifact_summary(artifact: &ExplorationArtifactSummary) -> String {
    format!(
        "saved artifact: {} [{}]\n",
        artifact.metadata.artifact_id,
        render_artifact_kind(artifact.kind)
    )
}

fn render_saved_review_summary(review: &ReviewRunSummary) -> String {
    format!(
        "saved review: {} [{}]\n",
        review.metadata.review_id,
        render_review_kind(review.kind)
    )
}

fn render_review_list(result: &ListReviewRunsResult) -> String {
    let mut output = String::new();
    if result.reviews.is_empty() {
        output.push_str("(none)\n");
        return output;
    }

    for review in &result.reviews {
        output.push_str(&format!(
            "- {} [{}]\n",
            review.metadata.title,
            render_review_kind(review.kind)
        ));
        output.push_str(&format!("  review id: {}\n", review.metadata.review_id));
        output.push_str(&format!("  findings: {}\n", review.finding_count));
        output.push_str(&format!(
            "  status: {}\n",
            render_review_status_counts(review)
        ));
        if let Some(summary) = &review.metadata.summary {
            output.push_str(&format!("  summary: {summary}\n"));
        }
    }
    output
}

fn render_review_run(review: &ReviewRun) -> String {
    let summary = ReviewRunSummary::from(review);
    let mut output = String::new();
    output.push_str(&format!("review id: {}\n", review.metadata.review_id));
    output.push_str(&format!("title: {}\n", review.metadata.title));
    output.push_str(&format!("kind: {}\n", render_review_kind(review.kind())));
    if let Some(summary_text) = &review.metadata.summary {
        output.push_str(&format!("summary: {summary_text}\n"));
    }
    output.push_str(&format!("findings: {}\n", summary.finding_count));
    output.push_str(&format!(
        "status: {}\n",
        render_review_status_counts(&summary)
    ));
    render_review_payload(&mut output, &review.payload);

    if review.findings.is_empty() {
        output.push_str("\n[findings]\n(none)\n");
        return output;
    }

    output.push_str("\n[findings]\n");
    for finding in &review.findings {
        render_review_finding(&mut output, finding, "");
    }
    output
}

fn render_review_diff(diff: &ReviewRunDiff) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "base review: {} [{}]\n",
        diff.base_review.metadata.review_id,
        render_review_kind(diff.base_review.kind)
    ));
    output.push_str(&format!(
        "target review: {} [{}]\n",
        diff.target_review.metadata.review_id,
        render_review_kind(diff.target_review.kind)
    ));
    output.push_str(&format!("added: {}\n", diff.added.len()));
    output.push_str(&format!("removed: {}\n", diff.removed.len()));
    output.push_str(&format!("unchanged: {}\n", diff.unchanged.len()));
    output.push_str(&format!(
        "content changed: {}\n",
        diff.content_changed.len()
    ));
    output.push_str(&format!("status changed: {}\n", diff.status_changed.len()));

    render_review_diff_findings(&mut output, "added", &diff.added);
    render_review_diff_findings(&mut output, "removed", &diff.removed);
    render_review_diff_pairs(&mut output, "unchanged", &diff.unchanged);
    render_review_diff_pairs(&mut output, "content-changed", &diff.content_changed);
    render_review_diff_status_changes(&mut output, &diff.status_changed);
    output
}

fn render_review_diff_findings(output: &mut String, section: &str, findings: &[ReviewFinding]) {
    if findings.is_empty() {
        return;
    }
    output.push_str(&format!("\n[{section}]\n"));
    for finding in findings {
        render_review_finding(output, finding, "");
    }
}

fn render_review_diff_pairs(output: &mut String, section: &str, pairs: &[ReviewFindingPair]) {
    if pairs.is_empty() {
        return;
    }
    output.push_str(&format!("\n[{section}]\n"));
    for pair in pairs {
        output.push_str(&format!("- {}\n", pair.finding_id));
        output.push_str("  base:\n");
        render_review_finding(output, &pair.base, "    ");
        output.push_str("  target:\n");
        render_review_finding(output, &pair.target, "    ");
    }
}

fn render_review_diff_status_changes(output: &mut String, changes: &[ReviewFindingStatusDiff]) {
    if changes.is_empty() {
        return;
    }
    output.push_str("\n[status-changed]\n");
    for change in changes {
        output.push_str(&format!("- {}\n", change.finding_id));
        output.push_str(&format!(
            "  status: {} -> {}\n",
            render_review_finding_status(change.from_status),
            render_review_finding_status(change.to_status)
        ));
        output.push_str("  target:\n");
        render_review_finding(output, &change.target, "    ");
    }
}

fn render_mark_review_finding_result(result: &MarkReviewFindingResult) -> String {
    format!(
        "marked review finding: {} {} {} -> {}\n",
        result.transition.review_id,
        result.transition.finding_id,
        render_review_finding_status(result.transition.from_status),
        render_review_finding_status(result.transition.to_status)
    )
}

fn render_review_remediation_preview(preview: &ReviewFindingRemediationPreview) -> String {
    let mut output = String::new();
    output.push_str(&format!("review id: {}\n", preview.review_id));
    output.push_str(&format!("finding id: {}\n", preview.finding_id));
    output.push_str(&format!(
        "status: {}\n",
        render_review_finding_status(preview.status)
    ));

    match &preview.payload {
        AuditRemediationPreviewPayload::DanglingLink {
            source,
            missing_explicit_id,
            file_path,
            line,
            column,
            preview: preview_text,
            suggestion,
            confidence,
            reason,
        } => {
            output.push_str("remediation: unlink-dangling-link\n");
            output.push_str(&format!(
                "confidence: {}\n",
                render_audit_remediation_confidence(*confidence)
            ));
            output.push_str(&format!("source: {}\n", render_anchor_identity(source)));
            output.push_str(&format!("missing id: {missing_explicit_id}\n"));
            output.push_str(&format!("location: {file_path}:{line}:{column}\n"));
            output.push_str(&format!("preview: {preview_text}\n"));
            output.push_str(&format!("suggestion: {suggestion}\n"));
            output.push_str(&format!("reason: {reason}\n"));
            output.push_str(&format!(
                "apply: slipbox review remediation apply {} {} --confirm-unlink-dangling-link\n",
                preview.review_id, preview.finding_id
            ));
        }
        AuditRemediationPreviewPayload::DuplicateTitle {
            title,
            notes,
            suggestion,
            confidence,
            reason,
        } => {
            output.push_str("remediation: manual-review\n");
            output.push_str(&format!(
                "confidence: {}\n",
                render_audit_remediation_confidence(*confidence)
            ));
            output.push_str(&format!("title: {title}\n"));
            output.push_str(&format!("notes: {}\n", notes.len()));
            for note in notes {
                output.push_str(&format!("  - {}\n", render_node_identity(note)));
            }
            output.push_str(&format!("suggestion: {suggestion}\n"));
            output.push_str(&format!("reason: {reason}\n"));
            output.push_str("apply: unsupported by safe remediation apply\n");
        }
    }
    output
}

fn render_review_remediation_application(result: &ReviewFindingRemediationApplyResult) -> String {
    let application = &result.application;
    let mut output = String::new();
    output.push_str(&format!(
        "applied remediation: {} {}\n",
        application.review_id, application.finding_id
    ));
    output.push_str(&format!(
        "action: {}\n",
        render_audit_remediation_apply_action(&application.action)
    ));
    output.push_str(&format!(
        "index: {}\n",
        render_structural_index_refresh(application.index_refresh)
    ));
    output.push_str("changed files:\n");
    if application.affected_files.changed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.changed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output.push_str("removed files:\n");
    if application.affected_files.removed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.removed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output.push_str(&format!(
        "inspect: slipbox review show {}\n",
        application.review_id
    ));
    output
}

fn render_slipbox_link_rewrite_preview(preview: &SlipboxLinkRewritePreview) -> String {
    let mut output = String::new();
    output.push_str(&format!("file: {}\n", preview.file_path));
    output.push_str(&format!("rewrites: {}\n", preview.rewrites.len()));
    if preview.rewrites.is_empty() {
        output.push_str("  (none)\n");
        return output;
    }
    for rewrite in &preview.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> ",
            rewrite.line, rewrite.column, rewrite.title_or_alias
        ));
        match &rewrite.target_explicit_id {
            Some(explicit_id) => output.push_str(&format!("id:{explicit_id}\n")),
            None => output.push_str("id will be assigned on apply\n"),
        }
        output.push_str(&format!(
            "  target: {}\n",
            render_node_identity(&rewrite.target)
        ));
        output.push_str(&format!("  description: {}\n", rewrite.description));
        output.push_str(&format!("  preview: {}\n", rewrite.preview));
        if let Some(replacement) = &rewrite.replacement {
            output.push_str(&format!("  replacement: {replacement}\n"));
        }
    }
    output
}

fn render_slipbox_link_rewrite_application(result: &SlipboxLinkRewriteApplyResult) -> String {
    let application = &result.application;
    let mut output = String::new();
    output.push_str(&format!(
        "rewrote slipbox links: {}\n",
        application.file_path
    ));
    output.push_str(&format!("rewrites: {}\n", application.rewrites.len()));
    for rewrite in &application.rewrites {
        output.push_str(&format!(
            "- {}:{} slipbox:{} -> id:{}\n",
            rewrite.line, rewrite.column, rewrite.title_or_alias, rewrite.target_explicit_id
        ));
        output.push_str(&format!("  target node: {}\n", rewrite.target_node_key));
        output.push_str(&format!("  replacement: {}\n", rewrite.replacement));
    }
    output.push_str(&format!(
        "index: {}\n",
        render_structural_index_refresh(application.index_refresh)
    ));
    output.push_str("changed files:\n");
    for file in &application.affected_files.changed_files {
        output.push_str(&format!("  - {file}\n"));
    }
    output.push_str("removed files:\n");
    if application.affected_files.removed_files.is_empty() {
        output.push_str("  (none)\n");
    } else {
        for file in &application.affected_files.removed_files {
            output.push_str(&format!("  - {file}\n"));
        }
    }
    output
}

fn render_audit_remediation_apply_action(action: &AuditRemediationApplyAction) -> &'static str {
    match action {
        AuditRemediationApplyAction::UnlinkDanglingLink { .. } => "unlink-dangling-link",
    }
}

fn render_audit_remediation_confidence(confidence: AuditRemediationConfidence) -> &'static str {
    match confidence {
        AuditRemediationConfidence::Low => "low",
        AuditRemediationConfidence::Medium => "medium",
        AuditRemediationConfidence::High => "high",
    }
}

fn render_review_payload(output: &mut String, payload: &ReviewRunPayload) {
    match payload {
        ReviewRunPayload::Audit { audit, limit } => {
            output.push_str(&format!("audit: {}\n", render_corpus_audit_kind(*audit)));
            output.push_str(&format!("limit: {limit}\n"));
        }
        ReviewRunPayload::Workflow {
            workflow,
            inputs,
            step_ids,
        } => {
            output.push_str(&format!(
                "workflow: {} [{}]\n",
                workflow.metadata.title, workflow.metadata.workflow_id
            ));
            output.push_str(&format!("steps: {}\n", workflow.step_count));
            output.push_str(&format!("source step ids: {}\n", step_ids.join(", ")));
            if inputs.is_empty() {
                output.push_str("inputs: 0\n");
            } else {
                output.push_str(&format!("inputs: {}\n", inputs.len()));
                for input in inputs {
                    output.push_str(&format!(
                        "  {}: {}\n",
                        input.input_id,
                        render_workflow_resolve_target(&input.target)
                    ));
                }
            }
        }
    }
}

fn render_review_finding(output: &mut String, finding: &ReviewFinding, indent: &str) {
    output.push_str(&format!(
        "{indent}- {} [{}]\n",
        finding.finding_id,
        render_review_finding_kind(finding.kind())
    ));
    output.push_str(&format!(
        "{indent}  status: {}\n",
        render_review_finding_status(finding.status)
    ));
    let payload = render_review_finding_payload_block(&finding.payload);
    push_indented(output, &payload, indent);
}

fn render_review_finding_payload(output: &mut String, payload: &ReviewFindingPayload) {
    match payload {
        ReviewFindingPayload::Audit { entry } => {
            render_review_audit_entry(output, entry);
        }
        ReviewFindingPayload::WorkflowStep { step } => {
            render_workflow_step_report(output, step);
        }
    }
}

fn render_review_finding_payload_block(payload: &ReviewFindingPayload) -> String {
    let mut output = String::new();
    render_review_finding_payload(&mut output, payload);
    output
}

fn push_indented(output: &mut String, text: &str, indent: &str) {
    for line in text.lines() {
        output.push_str(indent);
        output.push_str(line);
        output.push('\n');
    }
}

fn render_review_audit_entry(output: &mut String, entry: &CorpusAuditEntry) {
    match entry {
        CorpusAuditEntry::DanglingLink { record } => {
            output.push_str(&format!(
                "  dangling link: {} -> missing id {}\n",
                render_anchor_identity(&record.source),
                record.missing_explicit_id
            ));
            output.push_str(&format!(
                "  location: {}:{}:{}\n",
                record.source.file_path, record.line, record.column
            ));
            output.push_str(&format!("  preview: {}\n", record.preview));
        }
        CorpusAuditEntry::DuplicateTitle { record } => {
            output.push_str(&format!("  duplicate title: {}\n", record.title));
            output.push_str(&format!("  notes: {}\n", record.notes.len()));
        }
        CorpusAuditEntry::OrphanNote { record } => {
            output.push_str(&format!(
                "  orphan note: {} [{}]\n",
                record.note.title, record.note.node_key
            ));
            output.push_str(&format!(
                "  refs/backlinks/forward-links: {}/{}/{}\n",
                record.reference_count, record.backlink_count, record.forward_link_count
            ));
        }
        CorpusAuditEntry::WeaklyIntegratedNote { record } => {
            output.push_str(&format!(
                "  weakly integrated note: {} [{}]\n",
                record.note.title, record.note.node_key
            ));
            output.push_str(&format!(
                "  refs/backlinks/forward-links: {}/{}/{}\n",
                record.reference_count, record.backlink_count, record.forward_link_count
            ));
        }
    }
}

fn render_review_kind(kind: ReviewRunKind) -> &'static str {
    match kind {
        ReviewRunKind::Audit => "audit",
        ReviewRunKind::Workflow => "workflow",
    }
}

fn render_review_finding_kind(kind: ReviewFindingKind) -> &'static str {
    match kind {
        ReviewFindingKind::Audit => "audit",
        ReviewFindingKind::WorkflowStep => "workflow-step",
    }
}

fn render_review_finding_status(status: ReviewFindingStatus) -> &'static str {
    match status {
        ReviewFindingStatus::Open => "open",
        ReviewFindingStatus::Reviewed => "reviewed",
        ReviewFindingStatus::Dismissed => "dismissed",
        ReviewFindingStatus::Accepted => "accepted",
    }
}

fn render_review_status_counts(summary: &ReviewRunSummary) -> String {
    format!(
        "open/reviewed/dismissed/accepted: {}/{}/{}/{}",
        summary.status_counts.open,
        summary.status_counts.reviewed,
        summary.status_counts.dismissed,
        summary.status_counts.accepted
    )
}

fn render_index_stats(stats: &IndexStats) -> String {
    format!(
        "files indexed: {}\nnodes indexed: {}\nlinks indexed: {}\n",
        stats.files_indexed, stats.nodes_indexed, stats.links_indexed
    )
}

fn render_indexed_files(result: &IndexedFilesResult) -> String {
    let mut output = format!("indexed files: {}\n", result.files.len());
    for file_path in &result.files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output
}

fn render_file_search_result(result: &SearchFilesResult) -> String {
    let mut output = format!("files: {}\n", result.files.len());
    for file in &result.files {
        output.push_str(&render_file_record(file));
    }
    output
}

fn render_file_record(file: &FileRecord) -> String {
    format!(
        "- {} | {} | nodes: {}\n",
        file.file_path, file.title, file.node_count
    )
}

fn render_structural_write_report(report: &StructuralWriteReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("operation: {}\n", report.operation.label()));
    output.push_str(&format!(
        "index refresh: {}\n",
        render_structural_index_refresh(report.index_refresh)
    ));
    output.push_str(&format!(
        "changed files: {}\n",
        report.affected_files.changed_files.len()
    ));
    for file_path in &report.affected_files.changed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    output.push_str(&format!(
        "removed files: {}\n",
        report.affected_files.removed_files.len()
    ));
    for file_path in &report.affected_files.removed_files {
        output.push_str(&format!("- {file_path}\n"));
    }
    match &report.result {
        Some(StructuralWriteResult::Node { node }) => {
            output.push_str("result: node\n");
            output.push_str(&render_node_summary(node));
        }
        Some(StructuralWriteResult::Anchor { anchor }) => {
            output.push_str("result: anchor\n");
            output.push_str(&render_anchor_summary(anchor));
        }
        None => output.push_str("result: none\n"),
    }
    output
}

fn render_structural_index_refresh(
    status: slipbox_core::StructuralWriteIndexRefreshStatus,
) -> &'static str {
    match status {
        slipbox_core::StructuralWriteIndexRefreshStatus::Refreshed => "refreshed",
        slipbox_core::StructuralWriteIndexRefreshStatus::Pending => "pending",
    }
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

fn render_anchor_summary(anchor: &AnchorRecord) -> String {
    let mut output = String::new();
    output.push_str(&format!("anchor key: {}\n", anchor.node_key));
    if let Some(explicit_id) = &anchor.explicit_id {
        output.push_str(&format!("id: {explicit_id}\n"));
    }
    output.push_str(&format!("title: {}\n", anchor.title));
    output.push_str(&format!("kind: {}\n", anchor.kind.as_str()));
    output.push_str(&format!("file: {}\n", anchor.file_path));
    output.push_str(&format!("line: {}\n", anchor.line));
    if !anchor.outline_path.is_empty() {
        output.push_str(&format!("outline path: {}\n", anchor.outline_path));
    }
    if !anchor.aliases.is_empty() {
        output.push_str(&format!("aliases: {}\n", anchor.aliases.join(", ")));
    }
    if !anchor.refs.is_empty() {
        output.push_str(&format!("refs: {}\n", anchor.refs.join(", ")));
    }
    if !anchor.tags.is_empty() {
        output.push_str(&format!("tags: {}\n", anchor.tags.join(", ")));
    }
    if let Some(todo_keyword) = &anchor.todo_keyword {
        output.push_str(&format!("todo: {todo_keyword}\n"));
    }
    if let Some(scheduled_for) = &anchor.scheduled_for {
        output.push_str(&format!("scheduled: {scheduled_for}\n"));
    }
    if let Some(deadline_for) = &anchor.deadline_for {
        output.push_str(&format!("deadline: {deadline_for}\n"));
    }
    if let Some(closed_at) = &anchor.closed_at {
        output.push_str(&format!("closed: {closed_at}\n"));
    }
    output
}

fn render_node_search_result(result: &SearchNodesResult) -> String {
    let mut output = format!("nodes: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_node_identity(node)));
    }
    output
}

fn render_random_node_result(result: &RandomNodeResult) -> String {
    match &result.node {
        Some(node) => render_node_summary(node),
        None => "node: none\n".to_owned(),
    }
}

fn render_backlinks_result(result: &BacklinksResult) -> String {
    let mut output = format!("backlinks: {}\n", result.backlinks.len());
    for record in &result.backlinks {
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
    }
    output
}

fn render_forward_links_result(result: &ForwardLinksResult) -> String {
    let mut output = format!("forward links: {}\n", result.forward_links.len());
    for record in &result.forward_links {
        output.push_str(&format!(
            "- {} at {}:{}\n",
            render_node_identity(&record.destination_note),
            record.row,
            record.col
        ));
        output.push_str(&format!("  preview: {}\n", record.preview));
    }
    output
}

fn render_capture_preview(preview: &CaptureTemplatePreviewResult) -> String {
    let mut output = format!("preview file: {}\n", preview.file_path);
    output.push_str(&format!(
        "preview node: {} | {} | line {}\n",
        preview.preview_node.node_key, preview.preview_node.title, preview.preview_node.line
    ));
    output.push_str("--- content ---\n");
    output.push_str(&preview.content);
    if !preview.content.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn render_ref_search_result(result: &SearchRefsResult) -> String {
    let mut output = format!("refs: {}\n", result.refs.len());
    for record in &result.refs {
        output.push_str(&render_ref_record(record));
    }
    output
}

fn render_ref_record(record: &RefRecord) -> String {
    format!(
        "- {} -> {}\n",
        record.reference,
        render_node_identity(&record.node)
    )
}

fn render_tag_search_result(result: &SearchTagsResult) -> String {
    let mut output = format!("tags: {}\n", result.tags.len());
    for tag in &result.tags {
        output.push_str(&format!("- {tag}\n"));
    }
    output
}

fn render_occurrence_search_result(result: &SearchOccurrencesResult) -> String {
    let mut output = format!("occurrences: {}\n", result.occurrences.len());
    for record in &result.occurrences {
        output.push_str(&render_occurrence_record(record));
    }
    output
}

fn render_occurrence_record(record: &OccurrenceRecord) -> String {
    let mut output = format!("- {}:{}:{}\n", record.file_path, record.row, record.col);
    if let Some(anchor) = &record.owning_anchor {
        output.push_str(&format!("  anchor: {}\n", render_anchor_identity(anchor)));
    }
    output.push_str(&format!("  matched text: {}\n", record.matched_text));
    output.push_str(&format!("  preview: {}\n", record.preview));
    output
}

fn render_agenda_result(result: &AgendaResult) -> String {
    let mut output = format!("agenda entries: {}\n", result.nodes.len());
    for node in &result.nodes {
        output.push_str(&format!("- {}\n", render_anchor_identity(node)));
        if let Some(todo_keyword) = &node.todo_keyword {
            output.push_str(&format!("  todo: {todo_keyword}\n"));
        }
        if let Some(scheduled_for) = &node.scheduled_for {
            output.push_str(&format!("  scheduled: {scheduled_for}\n"));
        }
        if let Some(deadline_for) = &node.deadline_for {
            output.push_str(&format!("  deadline: {deadline_for}\n"));
        }
        if let Some(closed_at) = &node.closed_at {
            output.push_str(&format!("  closed: {closed_at}\n"));
        }
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

fn render_artifact_list(result: &ListExplorationArtifactsResult) -> String {
    let mut output = String::new();
    if result.artifacts.is_empty() {
        output.push_str("(none)\n");
        return output;
    }

    for artifact in &result.artifacts {
        output.push_str(&format!(
            "- {} [{}]\n",
            artifact.metadata.title,
            render_artifact_kind(artifact.kind)
        ));
        output.push_str(&format!(
            "  artifact id: {}\n",
            artifact.metadata.artifact_id
        ));
        if let Some(summary) = &artifact.metadata.summary {
            output.push_str(&format!("  summary: {summary}\n"));
        }
    }
    output
}

fn render_saved_exploration_artifact(artifact: &SavedExplorationArtifact) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        slipbox_core::ExplorationArtifactPayload::LensView { artifact } => {
            render_saved_lens_view_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Comparison { artifact } => {
            render_saved_comparison_artifact(&mut output, artifact);
        }
        slipbox_core::ExplorationArtifactPayload::Trail { artifact } => {
            render_saved_trail_artifact(&mut output, artifact);
        }
    }
    output
}

fn render_executed_exploration_artifact(artifact: &ExecutedExplorationArtifact) -> String {
    let mut output = String::new();
    render_artifact_metadata(&mut output, &artifact.metadata, artifact.kind());
    match &artifact.payload {
        ExecutedExplorationArtifactPayload::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        ExecutedExplorationArtifactPayload::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(&mut output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
        ExecutedExplorationArtifactPayload::Trail { artifact, replay } => {
            render_saved_trail_state(&mut output, artifact);
            output.push('\n');
            output.push_str("[replay]\n");
            output.push_str(&render_trail_replay_result(replay));
        }
    }
    output
}

fn render_artifact_metadata(
    output: &mut String,
    metadata: &slipbox_core::ExplorationArtifactMetadata,
    kind: ExplorationArtifactKind,
) {
    output.push_str(&format!("artifact id: {}\n", metadata.artifact_id));
    output.push_str(&format!("title: {}\n", metadata.title));
    output.push_str(&format!("kind: {}\n", render_artifact_kind(kind)));
    if let Some(summary) = &metadata.summary {
        output.push_str(&format!("summary: {summary}\n"));
    }
}

fn render_saved_lens_view_artifact(output: &mut String, artifact: &SavedLensViewArtifact) {
    render_saved_lens_view_state(output, artifact, "");
}

fn render_saved_lens_view_state(
    output: &mut String,
    artifact: &SavedLensViewArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}current node key: {}\n",
        label_prefix, artifact.current_node_key
    ));
    output.push_str(&format!(
        "{}lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.lens)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!("{}unique: {}\n", label_prefix, artifact.unique));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

fn render_saved_comparison_artifact(output: &mut String, artifact: &SavedComparisonArtifact) {
    render_saved_comparison_state(output, artifact, "");
}

fn render_saved_comparison_state(
    output: &mut String,
    artifact: &SavedComparisonArtifact,
    label_prefix: &str,
) {
    output.push_str(&format!(
        "{}root node key: {}\n",
        label_prefix, artifact.root_node_key
    ));
    output.push_str(&format!(
        "{}left node key: {}\n",
        label_prefix, artifact.left_node_key
    ));
    output.push_str(&format!(
        "{}right node key: {}\n",
        label_prefix, artifact.right_node_key
    ));
    output.push_str(&format!(
        "{}active lens: {}\n",
        label_prefix,
        render_exploration_lens(artifact.active_lens)
    ));
    output.push_str(&format!(
        "{}comparison group: {}\n",
        label_prefix,
        render_comparison_group(artifact.comparison_group)
    ));
    output.push_str(&format!("{}limit: {}\n", label_prefix, artifact.limit));
    output.push_str(&format!(
        "{}structure unique: {}\n",
        label_prefix, artifact.structure_unique
    ));
    output.push_str(&format!(
        "{}frozen context: {}\n",
        label_prefix, artifact.frozen_context
    ));
}

fn render_saved_trail_artifact(output: &mut String, artifact: &SavedTrailArtifact) {
    render_saved_trail_state(output, artifact);
    for (index, step) in artifact.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_saved_trail_step(output, step);
    }
    if let Some(step) = &artifact.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_saved_trail_step(output, step);
    }
}

fn render_saved_trail_state(output: &mut String, artifact: &SavedTrailArtifact) {
    output.push_str(&format!("steps: {}\n", artifact.steps.len()));
    output.push_str(&format!("cursor: {}\n", artifact.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if artifact.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
}

fn render_saved_trail_step(output: &mut String, step: &SavedTrailStep) {
    match step {
        SavedTrailStep::LensView { artifact } => {
            output.push_str("kind: lens-view\n");
            render_saved_lens_view_state(output, artifact, "");
        }
        SavedTrailStep::Comparison { artifact } => {
            output.push_str("kind: comparison\n");
            render_saved_comparison_state(output, artifact, "");
        }
    }
}

fn render_trail_replay_result(replay: &TrailReplayResult) -> String {
    let mut output = String::new();
    output.push_str(&format!("steps: {}\n", replay.steps.len()));
    output.push_str(&format!("cursor: {}\n", replay.cursor));
    output.push_str(&format!(
        "detached step: {}\n",
        if replay.detached_step.is_some() {
            "present"
        } else {
            "none"
        }
    ));
    for (index, step) in replay.steps.iter().enumerate() {
        output.push('\n');
        output.push_str(&format!("[step {index}]\n"));
        render_trail_replay_step(&mut output, step);
    }
    if let Some(step) = &replay.detached_step {
        output.push('\n');
        output.push_str("[detached step]\n");
        render_trail_replay_step(&mut output, step);
    }
    output
}

fn render_trail_replay_step(output: &mut String, step: &TrailReplayStepResult) {
    match step {
        TrailReplayStepResult::LensView {
            artifact,
            root_note,
            current_note,
            result,
        } => {
            output.push_str("kind: lens-view\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            output.push_str(&format!(
                "current: {}\n",
                render_node_identity(current_note)
            ));
            render_saved_lens_view_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_explore_result(result));
        }
        TrailReplayStepResult::Comparison {
            artifact,
            root_note,
            result,
        } => {
            output.push_str("kind: comparison\n");
            output.push_str(&format!("root: {}\n", render_node_identity(root_note)));
            render_saved_comparison_state(output, artifact, "saved ");
            output.push('\n');
            output.push_str("[result]\n");
            output.push_str(&render_compare_result(result, NoteComparisonGroup::All));
        }
    }
}

fn render_artifact_kind(kind: ExplorationArtifactKind) -> &'static str {
    match kind {
        ExplorationArtifactKind::LensView => "lens-view",
        ExplorationArtifactKind::Comparison => "comparison",
        ExplorationArtifactKind::Trail => "trail",
    }
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
            version: "0.7.0".to_owned(),
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

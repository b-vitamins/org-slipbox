use super::output::{CliCommandError, write_output};
use super::render::notes::{
    render_anchor_summary, render_backlinks_result, render_capture_preview,
    render_forward_links_result, render_node_search_result, render_node_summary,
    render_random_node_result, render_structural_write_report,
};
use super::runtime::{
    HeadlessArgs, HeadlessCommand, ResolveTarget, ResolveTargetArgs, invalid_request_error,
    normalize_daily_file_path, normalize_edit_file_path, require_resolved_anchor,
    require_resolved_node, resolve_anchor_or_note_target_key, resolve_note_target,
    run_headless_command, validate_region_range,
};
use anyhow::{Context, Result};
use chrono::{Local, NaiveDate};
use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use slipbox_core::{
    AnchorRecord, AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    BacklinksParams, BacklinksResult, CaptureContentType, CaptureNodeParams, CaptureTemplateParams,
    CaptureTemplatePreviewParams, EnsureFileNodeParams, EnsureNodeIdParams, ExtractSubtreeParams,
    ForwardLinksParams, ForwardLinksResult, NodeAtPointParams, NodeFromKeyParams, NodeRecord,
    RandomNodeResult, RefileRegionParams, RefileSubtreeParams, RewriteFileParams,
    SearchNodesParams, SearchNodesResult, StructuralWriteReport, UpdateNodeMetadataParams,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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
    #[arg(long, value_name = "DATE")]
    pub(crate) date: Option<String>,
    /// Daily note directory inside --root.
    #[arg(long, default_value = "daily", value_name = "DIR")]
    pub(crate) directory: String,
    /// strftime-compatible daily note filename format.
    #[arg(
        long = "file-format",
        default_value = "%Y-%m-%d.org",
        value_name = "FORMAT"
    )]
    pub(crate) file_format: String,
    /// strftime-compatible daily note title format.
    #[arg(
        long = "title-format",
        default_value = "%Y-%m-%d",
        value_name = "FORMAT"
    )]
    pub(crate) title_format: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct DailyEnsureArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: DailyTargetArgs,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long, value_name = "ORG")]
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
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
    /// Org heading level.
    #[arg(long, default_value_t = 1)]
    pub(crate) level: u32,
    /// Optional strftime-compatible file head used when creating the daily note.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DailyTarget {
    pub(crate) date: NaiveDate,
    pub(crate) file_path: String,
    pub(crate) title: String,
}

impl DailyTarget {
    pub(crate) fn node_key(&self) -> String {
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

#[derive(Debug, Clone, Args)]
pub(crate) struct CaptureArgs {
    #[command(subcommand)]
    pub(crate) command: CaptureCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum CaptureCommand {
    /// Write content through the daemon-owned capture-template engine.
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
#[command(group(
    ArgGroup::new("content-source")
        .args(["content", "content_file", "content_stdin"])
        .multiple(false)
))]
pub(crate) struct CaptureTemplateFields {
    /// Optional title used by file creation and entry captures.
    #[arg(long, default_value = "", value_name = "TITLE")]
    pub(crate) title: String,
    /// File target, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: Option<PathBuf>,
    /// Exact target node key.
    #[arg(long = "node-key", value_name = "KEY")]
    pub(crate) node_key: Option<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
    /// One or more outline path segments.
    #[arg(long = "outline", num_args = 1.., value_name = "HEADING")]
    pub(crate) outline_path: Vec<String>,
    /// Capture content type.
    #[arg(long = "type", value_enum, default_value_t = CaptureTypeArg::Plain)]
    pub(crate) capture_type: CaptureTypeArg,
    /// Capture content supplied directly on the command line.
    #[arg(long, value_name = "TEXT")]
    pub(crate) content: Option<String>,
    /// Read capture content from a file.
    #[arg(long = "content-file", value_name = "PATH")]
    pub(crate) content_file: Option<PathBuf>,
    /// Read capture content from stdin.
    #[arg(long = "content-stdin")]
    pub(crate) content_stdin: bool,
    /// Reference to attach when creating a new target file.
    #[arg(long = "ref", num_args = 1.., value_name = "REF")]
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
    #[arg(long = "table-line-pos", value_name = "POSITION")]
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
    #[arg(long = "source", value_name = "TEXT")]
    pub(crate) source_override: Option<String>,
    /// Read source text to preview against from a file.
    #[arg(long = "source-file", value_name = "PATH")]
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
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
    /// File path to create, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: Option<PathBuf>,
    /// Optional leading Org content to preserve before generated metadata.
    #[arg(long, value_name = "ORG")]
    pub(crate) head: Option<String>,
    /// Reference to attach to the created note.
    #[arg(long = "ref", num_args = 1.., value_name = "REF")]
    pub(crate) refs: Vec<String>,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteEnsureFileArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to ensure, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// File note title.
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendHeadingArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// File note title used when the file must be created.
    #[arg(long, value_name = "TITLE")]
    pub(crate) title: String,
    /// Heading title to append.
    #[arg(long, value_name = "HEADING")]
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
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
}

#[derive(Debug, Clone, Args)]
pub(crate) struct NoteAppendOutlineArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    /// File path to append within, absolute or relative to --root.
    #[arg(long, value_name = "FILE")]
    pub(crate) file: PathBuf,
    /// Heading title to append under the outline path.
    #[arg(long, value_name = "HEADING")]
    pub(crate) heading: String,
    /// One or more outline path segments.
    #[arg(long = "outline", required = true, num_args = 1.., value_name = "HEADING")]
    pub(crate) outline_path: Vec<String>,
    /// Optional leading Org content used when the file must be created.
    #[arg(long, value_name = "ORG")]
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

#[derive(Debug, Clone, Args)]
pub(crate) struct ResolveNodeArgs {
    #[command(flatten)]
    pub(crate) headless: HeadlessArgs,
    #[command(flatten)]
    pub(crate) target: ResolveTargetArgs,
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

pub(crate) fn run_daily(args: &DailyArgs) -> Result<(), CliCommandError> {
    match &args.command {
        DailyCommand::Ensure(command) => run_headless_command(command),
        DailyCommand::Show(command) => run_headless_command(command),
        DailyCommand::Append(command) => run_headless_command(command),
    }
}

pub(crate) fn run_capture(args: &CaptureArgs) -> Result<(), CliCommandError> {
    match &args.command {
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

pub(crate) fn run_resolve_node(args: &ResolveNodeArgs) -> Result<(), CliCommandError> {
    run_headless_command(args)
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

fn parse_daily_date(value: &str) -> Result<NaiveDate, DaemonClientError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        invalid_request_error(format!(
            "invalid daily date {value:?}: expected ISO date YYYY-MM-DD"
        ))
    })
}

fn today_local_date() -> NaiveDate {
    Local::now().date_naive()
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

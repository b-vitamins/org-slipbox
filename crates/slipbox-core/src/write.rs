use serde::{Deserialize, Serialize};

use crate::{
    nodes::{AnchorRecord, NodeRecord, PreviewNodeRecord},
    validation::{
        default_heading_level, normalize_reference, normalize_string_values,
        validate_positive_position, validate_required_text_field,
        validate_structural_write_file_path_field, validate_structural_write_file_paths,
        validate_structural_write_operation_files,
        validate_structural_write_preview_result_requirement,
        validate_structural_write_result_anchor, validate_structural_write_result_file,
        validate_structural_write_result_node, validate_structural_write_result_requirement,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureNodeParams {
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
}

impl CaptureNodeParams {
    #[must_use]
    pub fn normalized_refs(&self) -> Vec<String> {
        let mut refs: Vec<String> = Vec::new();

        for reference in &self.refs {
            for normalized in normalize_reference(reference) {
                if normalized.is_empty()
                    || refs
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
                {
                    continue;
                }
                refs.push(normalized);
            }
        }

        refs
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureContentType {
    Plain,
    Entry,
    Item,
    Checkitem,
    TableLine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplateParams {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
    #[serde(default)]
    pub node_key: Option<String>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub outline_path: Vec<String>,
    pub capture_type: CaptureContentType,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub prepend: bool,
    #[serde(default)]
    pub empty_lines_before: u32,
    #[serde(default)]
    pub empty_lines_after: u32,
    #[serde(default)]
    pub table_line_pos: Option<String>,
}

impl CaptureTemplateParams {
    #[must_use]
    pub fn normalized_outline_path(&self) -> Vec<String> {
        normalize_string_values(&self.outline_path, false)
    }

    #[must_use]
    pub fn normalized_refs(&self) -> Vec<String> {
        let mut refs: Vec<String> = Vec::new();

        for reference in &self.refs {
            for normalized in normalize_reference(reference) {
                if normalized.is_empty()
                    || refs
                        .iter()
                        .any(|existing| existing.eq_ignore_ascii_case(&normalized))
                {
                    continue;
                }
                refs.push(normalized);
            }
        }

        refs
    }

    #[must_use]
    pub fn normalized_empty_lines_before(&self) -> usize {
        self.empty_lines_before.min(8) as usize
    }

    #[must_use]
    pub fn normalized_empty_lines_after(&self) -> usize {
        self.empty_lines_after.min(8) as usize
    }

    #[must_use]
    pub fn normalized_table_line_pos(&self) -> Option<String> {
        self.table_line_pos.as_ref().and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplatePreviewParams {
    #[serde(flatten)]
    pub capture: CaptureTemplateParams,
    #[serde(default)]
    pub source_override: Option<String>,
    #[serde(default)]
    pub ensure_node_id: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureTemplatePreviewResult {
    pub file_path: String,
    pub content: String,
    pub preview_node: PreviewNodeRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsureFileNodeParams {
    pub file_path: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingParams {
    pub file_path: String,
    pub title: String,
    pub heading: String,
    #[serde(default = "default_heading_level")]
    pub level: u32,
}

impl AppendHeadingParams {
    #[must_use]
    pub fn normalized_level(&self) -> usize {
        self.level.clamp(1, 32) as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingToNodeParams {
    pub node_key: String,
    pub heading: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppendHeadingAtOutlinePathParams {
    pub file_path: String,
    pub heading: String,
    #[serde(default)]
    pub outline_path: Vec<String>,
    #[serde(default)]
    pub head: Option<String>,
}

impl AppendHeadingAtOutlinePathParams {
    #[must_use]
    pub fn normalized_outline_path(&self) -> Vec<String> {
        normalize_string_values(&self.outline_path, false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsureNodeIdParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateNodeMetadataParams {
    pub node_key: String,
    #[serde(default)]
    pub aliases: Option<Vec<String>>,
    #[serde(default)]
    pub refs: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

impl UpdateNodeMetadataParams {
    #[must_use]
    pub fn normalized_aliases(&self) -> Option<Vec<String>> {
        self.aliases
            .as_ref()
            .map(|values| normalize_string_values(values, false))
    }

    #[must_use]
    pub fn normalized_refs(&self) -> Option<Vec<String>> {
        self.refs.as_ref().map(|values| {
            let mut refs = Vec::new();
            for value in values {
                for normalized in normalize_reference(value) {
                    if normalized.is_empty()
                        || refs
                            .iter()
                            .any(|existing: &String| existing.eq_ignore_ascii_case(&normalized))
                    {
                        continue;
                    }
                    refs.push(normalized);
                }
            }
            refs
        })
    }

    #[must_use]
    pub fn normalized_tags(&self) -> Option<Vec<String>> {
        self.tags
            .as_ref()
            .map(|values| normalize_string_values(values, false))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefileSubtreeParams {
    pub source_node_key: String,
    pub target_node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefileRegionParams {
    pub file_path: String,
    pub start: u32,
    pub end: u32,
    pub target_node_key: String,
}

impl RefileRegionParams {
    #[must_use]
    pub fn normalized_range(&self) -> (usize, usize) {
        let start = self.start.max(1) as usize;
        let end = self.end.max(1) as usize;
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractSubtreeParams {
    pub source_node_key: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralWriteOperationKind {
    RefileSubtree,
    RefileRegion,
    ExtractSubtree,
    PromoteFile,
    DemoteFile,
}

impl StructuralWriteOperationKind {
    #[must_use]
    pub const fn requires_result(self) -> bool {
        !matches!(self, Self::RefileRegion)
    }

    #[must_use]
    pub const fn permits_removed_files(self) -> bool {
        matches!(self, Self::RefileSubtree | Self::RefileRegion)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RefileSubtree => "refile-subtree",
            Self::RefileRegion => "refile-region",
            Self::ExtractSubtree => "extract-subtree",
            Self::PromoteFile => "promote-file",
            Self::DemoteFile => "demote-file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralWriteIndexRefreshStatus {
    Refreshed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralWriteAffectedFiles {
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub removed_files: Vec<String>,
}

impl StructuralWriteAffectedFiles {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        if self.changed_files.is_empty() {
            return Some("structural writes must include at least one changed file".to_owned());
        }

        validate_structural_write_file_paths(&self.changed_files, "changed_files")
            .or_else(|| validate_structural_write_file_paths(&self.removed_files, "removed_files"))
            .or_else(|| {
                self.changed_files
                    .iter()
                    .find(|changed| self.removed_files.iter().any(|removed| removed == *changed))
                    .map(|path| {
                        format!("structural write file {path} cannot be both changed and removed")
                    })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StructuralWriteResult {
    Node { node: Box<NodeRecord> },
    Anchor { anchor: Box<AnchorRecord> },
}

impl StructuralWriteResult {
    #[must_use]
    pub fn file_path(&self) -> &str {
        match self {
            Self::Node { node } => &node.file_path,
            Self::Anchor { anchor } => &anchor.file_path,
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::Node { node } => validate_structural_write_result_node(node),
            Self::Anchor { anchor } => validate_structural_write_result_anchor(anchor),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StructuralWritePreviewResult {
    ExistingNode { node_key: String },
    ExistingAnchor { node_key: String },
    File { file_path: String },
}

impl StructuralWritePreviewResult {
    #[must_use]
    pub fn file_path(&self) -> Option<&str> {
        match self {
            Self::ExistingNode { .. } | Self::ExistingAnchor { .. } => None,
            Self::File { file_path } => Some(file_path),
        }
    }

    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        match self {
            Self::ExistingNode { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::ExistingAnchor { node_key } => validate_required_text_field(node_key, "node_key"),
            Self::File { file_path } => {
                validate_structural_write_file_path_field(file_path, "file_path")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralWriteReport {
    pub operation: StructuralWriteOperationKind,
    #[serde(flatten)]
    pub affected_files: StructuralWriteAffectedFiles,
    pub index_refresh: StructuralWriteIndexRefreshStatus,
    #[serde(default)]
    pub result: Option<StructuralWriteResult>,
}

impl StructuralWriteReport {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.affected_files
            .validation_error()
            .or_else(|| {
                validate_structural_write_operation_files(self.operation, &self.affected_files)
            })
            .or_else(|| {
                (self.index_refresh != StructuralWriteIndexRefreshStatus::Refreshed).then(|| {
                    "structural write reports must be returned after index refresh".to_owned()
                })
            })
            .or_else(|| {
                validate_structural_write_result_requirement(self.operation, self.result.as_ref())
            })
            .or_else(|| {
                self.result
                    .as_ref()
                    .and_then(StructuralWriteResult::validation_error)
            })
            .or_else(|| {
                self.result.as_ref().and_then(|result| {
                    validate_structural_write_result_file(result.file_path(), &self.affected_files)
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralWritePreview {
    pub operation: StructuralWriteOperationKind,
    #[serde(flatten)]
    pub affected_files: StructuralWriteAffectedFiles,
    #[serde(default)]
    pub result: Option<StructuralWritePreviewResult>,
}

impl StructuralWritePreview {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.affected_files
            .validation_error()
            .or_else(|| {
                validate_structural_write_operation_files(self.operation, &self.affected_files)
            })
            .or_else(|| {
                validate_structural_write_preview_result_requirement(
                    self.operation,
                    self.result.as_ref(),
                )
            })
            .or_else(|| {
                self.result
                    .as_ref()
                    .and_then(StructuralWritePreviewResult::validation_error)
            })
            .or_else(|| {
                self.result
                    .as_ref()
                    .and_then(StructuralWritePreviewResult::file_path)
                    .and_then(|file_path| {
                        validate_structural_write_result_file(file_path, &self.affected_files)
                    })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewritePreviewParams {
    pub file_path: String,
}

impl SlipboxLinkRewritePreviewParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_structural_write_file_path_field(&self.file_path, "file_path")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewritePreviewEntry {
    pub line: u32,
    pub column: u32,
    pub preview: String,
    pub link_text: String,
    pub title_or_alias: String,
    pub description: String,
    pub target: NodeRecord,
    #[serde(default)]
    pub target_explicit_id: Option<String>,
    #[serde(default)]
    pub replacement: Option<String>,
}

impl SlipboxLinkRewritePreviewEntry {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_positive_position(self.line, "line")
            .or_else(|| validate_positive_position(self.column, "column"))
            .or_else(|| validate_required_text_field(&self.preview, "preview"))
            .or_else(|| validate_required_text_field(&self.link_text, "link_text"))
            .or_else(|| validate_required_text_field(&self.title_or_alias, "title_or_alias"))
            .or_else(|| validate_required_text_field(&self.description, "description"))
            .or_else(|| {
                validate_structural_write_result_node(&self.target)
                    .map(|error| format!("target is invalid: {error}"))
            })
            .or_else(|| {
                self.target_explicit_id.as_ref().and_then(|explicit_id| {
                    validate_required_text_field(explicit_id, "target_explicit_id")
                })
            })
            .or_else(|| {
                self.replacement.as_ref().and_then(|replacement| {
                    validate_required_text_field(replacement, "replacement")
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewritePreview {
    pub file_path: String,
    #[serde(default)]
    pub rewrites: Vec<SlipboxLinkRewritePreviewEntry>,
}

impl SlipboxLinkRewritePreview {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_structural_write_file_path_field(&self.file_path, "file_path").or_else(|| {
            self.rewrites.iter().enumerate().find_map(|(index, entry)| {
                entry
                    .validation_error()
                    .map(|error| format!("link rewrite preview entry {index} is invalid: {error}"))
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewritePreviewResult {
    pub preview: SlipboxLinkRewritePreview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewriteApplyParams {
    pub expected_preview: SlipboxLinkRewritePreview,
}

impl SlipboxLinkRewriteApplyParams {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        self.expected_preview.validation_error().or_else(|| {
            self.expected_preview
                .rewrites
                .is_empty()
                .then(|| "link rewrite apply requires at least one previewed rewrite".to_owned())
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewriteAppliedEntry {
    pub line: u32,
    pub column: u32,
    pub title_or_alias: String,
    pub target_node_key: String,
    pub target_explicit_id: String,
    pub replacement: String,
}

impl SlipboxLinkRewriteAppliedEntry {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_positive_position(self.line, "line")
            .or_else(|| validate_positive_position(self.column, "column"))
            .or_else(|| validate_required_text_field(&self.title_or_alias, "title_or_alias"))
            .or_else(|| validate_required_text_field(&self.target_node_key, "target_node_key"))
            .or_else(|| {
                validate_required_text_field(&self.target_explicit_id, "target_explicit_id")
            })
            .or_else(|| validate_required_text_field(&self.replacement, "replacement"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewriteApplication {
    pub file_path: String,
    #[serde(default)]
    pub rewrites: Vec<SlipboxLinkRewriteAppliedEntry>,
    #[serde(flatten)]
    pub affected_files: StructuralWriteAffectedFiles,
    pub index_refresh: StructuralWriteIndexRefreshStatus,
}

impl SlipboxLinkRewriteApplication {
    #[must_use]
    pub fn validation_error(&self) -> Option<String> {
        validate_structural_write_file_path_field(&self.file_path, "file_path")
            .or_else(|| {
                self.rewrites.is_empty().then(|| {
                    "link rewrite applications must include at least one rewrite".to_owned()
                })
            })
            .or_else(|| {
                self.rewrites.iter().enumerate().find_map(|(index, entry)| {
                    entry.validation_error().map(|error| {
                        format!("link rewrite application entry {index} is invalid: {error}")
                    })
                })
            })
            .or_else(|| self.affected_files.validation_error())
            .or_else(|| {
                (self.index_refresh != StructuralWriteIndexRefreshStatus::Refreshed).then(|| {
                    "link rewrite applications must be returned after index refresh".to_owned()
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlipboxLinkRewriteApplyResult {
    pub application: SlipboxLinkRewriteApplication,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewriteFileParams {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFileParams {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFileResult {
    pub file_path: String,
}

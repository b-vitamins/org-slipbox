use serde::{Deserialize, Serialize};

use crate::{
    nodes::{AnchorRecord, NodeKind},
    validation::default_search_limit,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingInfo {
    pub version: String,
    pub root: String,
    pub db: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    pub version: String,
    pub root: String,
    pub db: String,
    pub files_indexed: u64,
    pub nodes_indexed: u64,
    pub links_indexed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFilesResult {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileDiagnosticIssue {
    MissingFromIndex,
    IndexedButMissing,
    IndexedButIneligible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiagnosticsParams {
    pub file_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiagnostics {
    pub file_path: String,
    pub absolute_path: String,
    pub exists: bool,
    pub eligible: bool,
    pub indexed: bool,
    #[serde(default)]
    pub index_record: Option<FileRecord>,
    #[serde(default)]
    pub issues: Vec<FileDiagnosticIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiagnosticsResult {
    pub diagnostic: FileDiagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeDiagnosticIssue {
    SourceFileMissing,
    SourceFileIneligible,
    SourceFileUnindexed,
    LineOutOfRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDiagnosticsParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDiagnostics {
    pub node: AnchorRecord,
    pub file: FileDiagnostics,
    pub line_present: bool,
    #[serde(default)]
    pub issues: Vec<NodeDiagnosticIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDiagnosticsResult {
    pub diagnostic: NodeDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDiagnostics {
    pub root: String,
    pub eligible_files: Vec<String>,
    pub indexed_files: Vec<String>,
    pub missing_from_index: Vec<String>,
    pub indexed_but_missing: Vec<String>,
    pub indexed_but_ineligible: Vec<String>,
    pub status: IndexStats,
    pub status_consistent: bool,
    pub index_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexDiagnosticsResult {
    pub diagnostic: IndexDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_path: String,
    pub title: String,
    pub mtime_ns: i64,
    pub node_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFile {
    pub file_path: String,
    pub title: String,
    pub mtime_ns: i64,
    pub nodes: Vec<IndexedNode>,
    pub links: Vec<IndexedLink>,
    pub occurrence_document: Option<IndexedOccurrenceDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedNode {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    pub aliases: Vec<String>,
    pub tags: Vec<String>,
    pub refs: Vec<String>,
    pub todo_keyword: Option<String>,
    pub scheduled_for: Option<String>,
    pub deadline_for: Option<String>,
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedLink {
    pub source_node_key: String,
    pub destination_explicit_id: String,
    pub line: u32,
    pub column: u32,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedOccurrenceDocument {
    pub file_path: String,
    pub search_text: String,
    pub line_rows: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStats {
    pub files_indexed: u64,
    pub nodes_indexed: u64,
    pub links_indexed: u64,
}

impl IndexStats {
    pub fn accumulate(&mut self, other: &Self) {
        self.files_indexed += other.files_indexed;
        self.nodes_indexed += other.nodes_indexed;
        self.links_indexed += other.links_indexed;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilesParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

impl SearchFilesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchFilesResult {
    pub files: Vec<FileRecord>,
}

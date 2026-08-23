use serde::{Deserialize, Serialize};

use crate::{AnchorRecord, BacklinkRecord, ForwardLinkRecord, NodeRecord};

const DEFAULT_SOURCE_MAX_LINES: usize = 200;
const MAX_SOURCE_LINES: usize = 1_000;
const MAX_CONTEXT_LINES: u32 = 200;
const DEFAULT_CONTEXT_RELATION_LIMIT: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorFromKeyParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileSourceParams {
    pub file_path: String,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub max_lines: Option<usize>,
}

impl ReadFileSourceParams {
    #[must_use]
    pub fn normalized_start_line(&self) -> u32 {
        self.start_line.unwrap_or(1).max(1)
    }

    #[must_use]
    pub fn normalized_max_lines(&self) -> usize {
        normalize_source_line_limit(self.max_lines)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadNodeSourceParams {
    pub node_key: String,
    #[serde(default)]
    pub context_before: Option<u32>,
    #[serde(default)]
    pub context_after: Option<u32>,
    #[serde(default)]
    pub max_lines: Option<usize>,
}

impl ReadNodeSourceParams {
    #[must_use]
    pub fn normalized_context_before(&self) -> u32 {
        normalize_context_lines(self.context_before)
    }

    #[must_use]
    pub fn normalized_context_after(&self) -> u32 {
        normalize_context_lines(self.context_after)
    }

    #[must_use]
    pub fn normalized_max_lines(&self) -> usize {
        normalize_source_line_limit(self.max_lines)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteContextParams {
    pub node_key: String,
    #[serde(default)]
    pub source_context_before: Option<u32>,
    #[serde(default)]
    pub source_context_after: Option<u32>,
    #[serde(default)]
    pub source_max_lines: Option<usize>,
    #[serde(default)]
    pub relation_limit: Option<usize>,
}

impl NoteContextParams {
    #[must_use]
    pub fn normalized_source_context_before(&self) -> u32 {
        normalize_context_lines(self.source_context_before)
    }

    #[must_use]
    pub fn normalized_source_context_after(&self) -> u32 {
        normalize_context_lines(self.source_context_after)
    }

    #[must_use]
    pub fn normalized_source_max_lines(&self) -> usize {
        normalize_source_line_limit(self.source_max_lines)
    }

    #[must_use]
    pub fn normalized_relation_limit(&self) -> usize {
        self.relation_limit
            .unwrap_or(DEFAULT_CONTEXT_RELATION_LIMIT)
            .clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadFileSourceResult {
    pub source: SourceSlice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadNodeSourceResult {
    pub anchor: AnchorRecord,
    pub source: SourceSlice,
    pub node_start_line: u32,
    pub node_line_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteContextResult {
    pub note: NodeRecord,
    pub source: SourceSlice,
    pub node_start_line: u32,
    pub node_line_count: u32,
    pub backlinks: Vec<BacklinkRecord>,
    pub forward_links: Vec<ForwardLinkRecord>,
}

/// A note's place in `(file_path, line)` order, counted from 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotePlaceResult {
    pub ordinal: u64,
    pub total: u64,
    #[serde(default)]
    pub earlier: Option<NotePlaceNeighbor>,
    #[serde(default)]
    pub later: Option<NotePlaceNeighbor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotePlaceNeighbor {
    pub node_key: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSlice {
    pub file_path: String,
    pub start_line: u32,
    pub line_count: u32,
    pub total_lines: u32,
    pub content: String,
    pub truncated_before: bool,
    pub truncated_after: bool,
}

fn normalize_source_line_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_SOURCE_MAX_LINES)
        .clamp(1, MAX_SOURCE_LINES)
}

fn normalize_context_lines(lines: Option<u32>) -> u32 {
    lines.unwrap_or(0).min(MAX_CONTEXT_LINES)
}

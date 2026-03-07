use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PingInfo {
    pub version: String,
    pub root: String,
    pub db: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Heading,
}

impl NodeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Heading => "heading",
        }
    }
}

impl FromStr for NodeKind {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "file" => Ok(Self::File),
            "heading" => Ok(Self::Heading),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFile {
    pub file_path: String,
    pub mtime_ns: i64,
    pub nodes: Vec<IndexedNode>,
    pub links: Vec<IndexedLink>,
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
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedLink {
    pub source_node_key: String,
    pub destination_explicit_id: String,
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
pub struct SearchNodesParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

impl SearchNodesParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNodesResult {
    pub nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinksParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
}

impl BacklinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinksResult {
    pub backlinks: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureNodeParams {
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
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
pub struct EnsureNodeIdParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexFileParams {
    pub file_path: String,
}

const fn default_search_limit() -> usize {
    50
}

const fn default_backlink_limit() -> usize {
    200
}

const fn default_heading_level() -> u32 {
    1
}

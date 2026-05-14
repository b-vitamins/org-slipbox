use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{diagnostics::IndexedNode, validation::default_search_limit};

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
pub struct AnchorRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
    #[serde(default)]
    pub file_mtime_ns: i64,
    #[serde(default)]
    pub backlink_count: u64,
    #[serde(default)]
    pub forward_link_count: u64,
}

impl AnchorRecord {
    #[must_use]
    pub fn is_note(&self) -> bool {
        matches!(self.kind, NodeKind::File) || self.explicit_id.is_some()
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
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
    #[serde(default)]
    pub file_mtime_ns: i64,
    #[serde(default)]
    pub backlink_count: u64,
    #[serde(default)]
    pub forward_link_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewNodeRecord {
    pub node_key: String,
    pub explicit_id: Option<String>,
    pub file_path: String,
    pub title: String,
    pub outline_path: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub todo_keyword: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub deadline_for: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    pub level: u32,
    pub line: u32,
    pub kind: NodeKind,
}

impl From<IndexedNode> for NodeRecord {
    fn from(node: IndexedNode) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        }
    }
}

impl TryFrom<AnchorRecord> for NodeRecord {
    type Error = AnchorRecord;

    fn try_from(anchor: AnchorRecord) -> Result<Self, Self::Error> {
        if anchor.is_note() {
            Ok(Self {
                node_key: anchor.node_key,
                explicit_id: anchor.explicit_id,
                file_path: anchor.file_path,
                title: anchor.title,
                outline_path: anchor.outline_path,
                aliases: anchor.aliases,
                tags: anchor.tags,
                refs: anchor.refs,
                todo_keyword: anchor.todo_keyword,
                scheduled_for: anchor.scheduled_for,
                deadline_for: anchor.deadline_for,
                closed_at: anchor.closed_at,
                level: anchor.level,
                line: anchor.line,
                kind: anchor.kind,
                file_mtime_ns: anchor.file_mtime_ns,
                backlink_count: anchor.backlink_count,
                forward_link_count: anchor.forward_link_count,
            })
        } else {
            Err(anchor)
        }
    }
}

impl From<NodeRecord> for AnchorRecord {
    fn from(node: NodeRecord) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
            file_mtime_ns: node.file_mtime_ns,
            backlink_count: node.backlink_count,
            forward_link_count: node.forward_link_count,
        }
    }
}

impl From<IndexedNode> for PreviewNodeRecord {
    fn from(node: IndexedNode) -> Self {
        Self {
            node_key: node.node_key,
            explicit_id: node.explicit_id,
            file_path: node.file_path,
            title: node.title,
            outline_path: node.outline_path,
            aliases: node.aliases,
            tags: node.tags,
            refs: node.refs,
            todo_keyword: node.todo_keyword,
            scheduled_for: node.scheduled_for,
            deadline_for: node.deadline_for,
            closed_at: node.closed_at,
            level: node.level,
            line: node.line,
            kind: node.kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchNodesSort {
    Relevance,
    Title,
    File,
    FileMtime,
    BacklinkCount,
    ForwardLinkCount,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchNodesParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub sort: Option<SearchNodesSort>,
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
pub struct RandomNodeResult {
    pub node: Option<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromIdParams {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromKeyParams {
    pub node_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromTitleOrAliasParams {
    pub title_or_alias: String,
    #[serde(default)]
    pub nocase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeAtPointParams {
    pub file_path: String,
    pub line: u32,
}

impl NodeAtPointParams {
    #[must_use]
    pub fn normalized_line(&self) -> u32 {
        self.line.max(1)
    }
}

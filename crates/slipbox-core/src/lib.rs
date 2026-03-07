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
pub struct RandomNodeResult {
    pub node: Option<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTagsParams {
    pub query: String,
    #[serde(default = "default_tag_limit")]
    pub limit: usize,
}

impl SearchTagsParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTagsResult {
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromIdParams {
    pub id: String,
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
pub struct AgendaParams {
    pub start: String,
    pub end: String,
    #[serde(default = "default_agenda_limit")]
    pub limit: usize,
}

impl AgendaParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgendaResult {
    pub nodes: Vec<NodeRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefRecord {
    pub reference: String,
    pub node: NodeRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRefsParams {
    pub query: String,
    #[serde(default = "default_ref_limit")]
    pub limit: usize,
}

impl SearchRefsParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRefsResult {
    pub refs: Vec<RefRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeFromRefParams {
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureNodeParams {
    pub title: String,
    #[serde(default)]
    pub file_path: Option<String>,
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

const fn default_tag_limit() -> usize {
    200
}

const fn default_agenda_limit() -> usize {
    200
}

const fn default_ref_limit() -> usize {
    50
}

const fn default_heading_level() -> u32 {
    1
}

#[must_use]
pub fn normalize_reference(input: &str) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|value| value.strip_suffix("]]"))
    {
        let target = inner.split_once("][").map_or(inner, |(path, _)| path);
        return normalize_reference(target);
    }

    if let Some(key) = trimmed.strip_prefix('@') {
        return normalize_cite_keys(key);
    }

    if let Some(inner) = trimmed
        .strip_prefix("[cite:")
        .and_then(|value| value.strip_suffix(']'))
    {
        return extract_org_cite_keys(inner);
    }

    if let Some(path) = trimmed.strip_prefix("cite:") {
        return normalize_cite_keys(path);
    }

    vec![trimmed.to_owned()]
}

fn normalize_cite_keys(input: &str) -> Vec<String> {
    input
        .split([',', ';'])
        .filter_map(|part| {
            let key = part
                .trim()
                .trim_start_matches('@')
                .trim_start_matches("cite:")
                .trim();
            if key.is_empty() {
                None
            } else {
                Some(format!("@{key}"))
            }
        })
        .collect()
}

fn extract_org_cite_keys(input: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut current = String::new();
    let mut collecting = false;

    for character in input.chars() {
        if collecting {
            if is_cite_key_char(character) {
                current.push(character);
                continue;
            }
            if !current.is_empty() {
                refs.push(format!("@{current}"));
                current.clear();
            }
            collecting = false;
        }

        if character == '@' {
            collecting = true;
        }
    }

    if collecting && !current.is_empty() {
        refs.push(format!("@{current}"));
    }

    if refs.is_empty() {
        normalize_cite_keys(input)
    } else {
        refs
    }
}

fn is_cite_key_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
}

#[cfg(test)]
mod tests {
    use super::{CaptureNodeParams, normalize_reference};

    #[test]
    fn normalizes_common_reference_forms() {
        assert_eq!(normalize_reference("@thrun2005"), vec!["@thrun2005"]);
        assert_eq!(normalize_reference("cite:thrun2005"), vec!["@thrun2005"]);
        assert_eq!(
            normalize_reference("[cite:@thrun2005; @smith2024]"),
            vec!["@thrun2005", "@smith2024"]
        );
        assert_eq!(
            normalize_reference("[[https://example.test/path][Example]]"),
            vec!["https://example.test/path"]
        );
    }

    #[test]
    fn capture_params_normalize_and_deduplicate_refs() {
        let params = CaptureNodeParams {
            title: "Note".to_owned(),
            file_path: None,
            refs: vec![
                "cite:smith2024".to_owned(),
                "@smith2024".to_owned(),
                "https://example.test".to_owned(),
            ],
        };

        assert_eq!(
            params.normalized_refs(),
            vec!["@smith2024".to_owned(), "https://example.test".to_owned()]
        );
    }
}

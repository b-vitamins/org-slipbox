use std::str::FromStr;

use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_path: String,
    pub title: String,
    pub mtime_ns: i64,
    pub node_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphTitleShortening {
    Truncate,
    Wrap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphParams {
    #[serde(default)]
    pub root_node_key: Option<String>,
    #[serde(default)]
    pub max_distance: Option<u32>,
    #[serde(default)]
    pub include_orphans: bool,
    #[serde(default)]
    pub hidden_link_types: Vec<String>,
    #[serde(default = "default_graph_max_title_length")]
    pub max_title_length: usize,
    #[serde(default)]
    pub shorten_titles: Option<GraphTitleShortening>,
    #[serde(default)]
    pub node_url_prefix: Option<String>,
}

impl GraphParams {
    #[must_use]
    pub fn normalized_hidden_link_types(&self) -> Vec<String> {
        let mut types = Vec::new();
        for link_type in &self.hidden_link_types {
            let normalized = link_type.trim().to_ascii_lowercase();
            if !normalized.is_empty() && !types.iter().any(|candidate| candidate == &normalized) {
                types.push(normalized);
            }
        }
        types
    }

    #[must_use]
    pub fn normalized_max_title_length(&self) -> usize {
        self.max_title_length.clamp(8, 500)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResult {
    pub dot: String,
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
    #[serde(default)]
    pub file_mtime_ns: i64,
    #[serde(default)]
    pub backlink_count: u64,
    #[serde(default)]
    pub forward_link_count: u64,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedLink {
    pub source_node_key: String,
    pub destination_explicit_id: String,
    pub line: u32,
    pub column: u32,
    pub preview: String,
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
    #[serde(default)]
    pub unique: bool,
}

impl BacklinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinksResult {
    pub backlinks: Vec<BacklinkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BacklinkRecord {
    pub source_node: NodeRecord,
    pub row: u32,
    pub col: u32,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinksParams {
    pub node_key: String,
    #[serde(default = "default_backlink_limit")]
    pub limit: usize,
    #[serde(default)]
    pub unique: bool,
}

impl ForwardLinksParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 1_000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinksResult {
    pub forward_links: Vec<ForwardLinkRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardLinkRecord {
    pub destination_node: NodeRecord,
    pub row: u32,
    pub col: u32,
    pub preview: String,
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
    pub node: NodeRecord,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RewriteFileParams {
    pub file_path: String,
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

const fn default_graph_max_title_length() -> usize {
    100
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

fn normalize_string_values(values: &[String], nocase: bool) -> Vec<String> {
    let mut normalized = Vec::new();

    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty()
            || normalized.iter().any(|existing: &String| {
                if nocase {
                    existing.eq_ignore_ascii_case(trimmed)
                } else {
                    existing == trimmed
                }
            })
        {
            continue;
        }

        normalized.push(trimmed.to_owned());
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureNodeParams, NodeKind, NodeRecord, SearchNodesParams, SearchNodesSort,
        UpdateNodeMetadataParams, normalize_reference,
    };
    use serde_json::json;

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
            head: None,
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

    #[test]
    fn metadata_params_normalize_fields() {
        let params = UpdateNodeMetadataParams {
            node_key: "heading:note.org:3".to_owned(),
            aliases: Some(vec![
                " Bruce ".to_owned(),
                "Bruce".to_owned(),
                String::new(),
            ]),
            refs: Some(vec!["cite:smith2024".to_owned(), "@smith2024".to_owned()]),
            tags: Some(vec![
                "alpha".to_owned(),
                " alpha ".to_owned(),
                "beta".to_owned(),
            ]),
        };

        assert_eq!(params.normalized_aliases(), Some(vec!["Bruce".to_owned()]));
        assert_eq!(
            params.normalized_refs(),
            Some(vec!["@smith2024".to_owned()])
        );
        assert_eq!(
            params.normalized_tags(),
            Some(vec!["alpha".to_owned(), "beta".to_owned()])
        );
    }

    #[test]
    fn node_record_serialization_includes_metadata_fields() {
        let node = NodeRecord {
            node_key: "heading:note.org:3".to_owned(),
            explicit_id: Some("note-id".to_owned()),
            file_path: "note.org".to_owned(),
            title: "Note".to_owned(),
            outline_path: "Parent".to_owned(),
            aliases: vec!["Alias".to_owned()],
            tags: vec!["tag".to_owned()],
            refs: vec!["@smith2024".to_owned()],
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            level: 1,
            line: 3,
            kind: NodeKind::Heading,
            file_mtime_ns: 123,
            backlink_count: 2,
            forward_link_count: 4,
        };

        assert_eq!(
            serde_json::to_value(node).expect("node record should serialize"),
            json!({
                "node_key": "heading:note.org:3",
                "explicit_id": "note-id",
                "file_path": "note.org",
                "title": "Note",
                "outline_path": "Parent",
                "aliases": ["Alias"],
                "tags": ["tag"],
                "refs": ["@smith2024"],
                "todo_keyword": null,
                "scheduled_for": null,
                "deadline_for": null,
                "closed_at": null,
                "level": 1,
                "line": 3,
                "kind": "heading",
                "file_mtime_ns": 123,
                "backlink_count": 2,
                "forward_link_count": 4
            })
        );
    }

    #[test]
    fn search_nodes_params_support_kebab_case_sort_names() {
        let params: SearchNodesParams = serde_json::from_value(json!({
            "query": "alpha",
            "limit": 10,
            "sort": "forward-link-count"
        }))
        .expect("search node params should deserialize");

        assert_eq!(params.query, "alpha");
        assert_eq!(params.limit, 10);
        assert_eq!(params.sort, Some(SearchNodesSort::ForwardLinkCount));

        assert_eq!(
            serde_json::to_value(&params).expect("search node params should serialize"),
            json!({
                "query": "alpha",
                "limit": 10,
                "sort": "forward-link-count"
            })
        );
    }
}

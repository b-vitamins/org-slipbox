use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde_json::Value;

use slipbox_core::{AnchorRecord, NodeKind, NodeRecord, SearchNodesSort};

use crate::Database;

pub(crate) const ANCHOR_SELECT_COLUMN_COUNT: usize = 18;

impl Database {
    pub fn search_nodes(
        &self,
        query: &str,
        limit: usize,
        sort: Option<SearchNodesSort>,
    ) -> Result<Vec<NodeRecord>> {
        self.search_note_records(query, limit, sort)
    }

    pub fn search_anchors(
        &self,
        query: &str,
        limit: usize,
        sort: Option<SearchNodesSort>,
    ) -> Result<Vec<AnchorRecord>> {
        self.search_anchor_records(query, limit, sort)
    }

    fn search_note_records(
        &self,
        query: &str,
        limit: usize,
        sort: Option<SearchNodesSort>,
    ) -> Result<Vec<NodeRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        let note_where = note_where("n");
        if let Some(fts_query) = build_fts_query(query) {
            let sql = format!(
                "SELECT {}
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                    AND {}
                  ORDER BY {}
                  LIMIT ?2",
                anchor_select_columns("n"),
                note_where,
                search_nodes_order_by(sort.as_ref(), true)
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![fts_query, limit], row_to_note)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read note search results")
        } else {
            let sql = format!(
                "SELECT {}
                   FROM nodes AS n
                  WHERE {}
                  ORDER BY {}
                  LIMIT ?1",
                anchor_select_columns("n"),
                note_where,
                search_nodes_order_by(sort.as_ref(), false)
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![limit], row_to_note)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read note listing")
        }
    }

    fn search_anchor_records(
        &self,
        query: &str,
        limit: usize,
        sort: Option<SearchNodesSort>,
    ) -> Result<Vec<AnchorRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if let Some(fts_query) = build_fts_query(query) {
            let sql = format!(
                "SELECT {}
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                  ORDER BY {}
                  LIMIT ?2",
                anchor_select_columns("n"),
                search_nodes_order_by(sort.as_ref(), true)
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![fts_query, limit], row_to_anchor)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read anchor search results")
        } else {
            let sql = format!(
                "SELECT {}
                   FROM nodes AS n
                  ORDER BY {}
                  LIMIT ?1",
                anchor_select_columns("n"),
                search_nodes_order_by(sort.as_ref(), false)
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![limit], row_to_anchor)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read anchor listing")
        }
    }

    pub fn random_node(&self) -> Result<Option<NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
              ORDER BY random()
              LIMIT 1",
            anchor_select_columns("n"),
            note_where("n"),
        );
        self.connection
            .query_row(&sql, [], row_to_note)
            .optional()
            .context("failed to fetch random note")
    }

    pub fn search_tags(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let limit = limit.clamp(1, 1_000) as i64;
        if query.trim().is_empty() {
            let mut statement = self.connection.prepare(
                "SELECT DISTINCT tag
                   FROM tags
                  ORDER BY tag
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read tags")
        } else {
            let pattern = format!("{}%", escape_like_pattern(query.trim()));
            let mut statement = self.connection.prepare(
                "SELECT DISTINCT tag
                   FROM tags
                  WHERE tag LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                  ORDER BY tag
                  LIMIT ?2",
            )?;
            let rows =
                statement.query_map(params![pattern, limit], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to search tags")
        }
    }

    pub fn node_from_id(&self, explicit_id: &str) -> Result<Option<NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.explicit_id = ?1
              LIMIT 1",
            anchor_select_columns("n")
        );
        self.connection
            .query_row(&sql, params![explicit_id], row_to_note)
            .optional()
            .context("failed to fetch note from ID")
    }

    pub fn node_from_title_or_alias(
        &self,
        title_or_alias: &str,
        nocase: bool,
    ) -> Result<Vec<NodeRecord>> {
        let note_where = note_where("n");
        let sql = if nocase {
            format!(
                "SELECT DISTINCT {}
                   FROM nodes AS n
                   LEFT JOIN aliases AS a ON a.node_key = n.node_key
                  WHERE {}
                    AND (n.title = ?1 COLLATE NOCASE
                     OR a.alias = ?1 COLLATE NOCASE)
                  ORDER BY n.file_path, n.line
                  LIMIT 2",
                anchor_select_columns("n"),
                note_where,
            )
        } else {
            format!(
                "SELECT DISTINCT {}
                   FROM nodes AS n
                   LEFT JOIN aliases AS a ON a.node_key = n.node_key
                  WHERE {}
                    AND (n.title = ?1
                     OR a.alias = ?1)
                  ORDER BY n.file_path, n.line
                  LIMIT 2",
                anchor_select_columns("n"),
                note_where,
            )
        };
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![title_or_alias], row_to_note)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to fetch note from title or alias")
    }

    pub fn node_at_point(&self, file_path: &str, line: u32) -> Result<Option<NodeRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT n.node_key,
                    n.explicit_id,
                    n.kind,
                    n.level
               FROM nodes AS n
              WHERE n.file_path = ?1
                AND n.line <= ?2
              ORDER BY n.line DESC, n.level DESC",
        )?;
        let mut rows = statement.query(params![file_path, line])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let anchor = row_to_point_lookup_anchor(row)?;
        if anchor.is_note() {
            return self.note_by_key(&anchor.node_key);
        }

        let mut ancestor_level = anchor.level;
        while let Some(row) = rows.next()? {
            let candidate = row_to_point_lookup_anchor(row)?;
            if candidate.level >= ancestor_level {
                continue;
            }
            if candidate.is_note() {
                return self.note_by_key(&candidate.node_key);
            }
            ancestor_level = candidate.level;
        }

        Ok(None)
    }

    pub fn anchor_at_point(&self, file_path: &str, line: u32) -> Result<Option<AnchorRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.file_path = ?1
                AND n.line <= ?2
              ORDER BY n.line DESC, n.level DESC
              LIMIT 1",
            anchor_select_columns("n")
        );
        self.connection
            .query_row(&sql, params![file_path, line], row_to_anchor)
            .optional()
            .context("failed to fetch anchor at point")
    }

    pub fn note_by_key(&self, node_key: &str) -> Result<Option<NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.node_key = ?1
                AND {}",
            anchor_select_columns("n"),
            note_where("n"),
        );
        self.connection
            .query_row(&sql, params![node_key], row_to_note)
            .optional()
            .context("failed to fetch note by key")
    }

    pub fn anchor_by_key(&self, node_key: &str) -> Result<Option<AnchorRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.node_key = ?1",
            anchor_select_columns("n")
        );
        self.connection
            .query_row(&sql, params![node_key], row_to_anchor)
            .optional()
            .context("failed to fetch anchor by key")
    }

    pub fn anchors_in_file(&self, file_path: &str) -> Result<Vec<AnchorRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.file_path = ?1
              ORDER BY n.line, n.level",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![file_path], row_to_anchor)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read anchors for indexed file")
    }

    pub fn anchors_in_files(
        &self,
        file_paths: &[String],
    ) -> Result<HashMap<String, Vec<AnchorRecord>>> {
        if file_paths.is_empty() {
            return Ok(HashMap::new());
        }

        let placeholders = (1..=file_paths.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.file_path IN ({})
              ORDER BY n.file_path COLLATE NOCASE, n.file_path, n.line, n.level",
            anchor_select_columns("n"),
            placeholders
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(file_paths.iter()), row_to_anchor)?;
        let anchors = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read anchors for indexed files")?;

        let mut grouped = HashMap::new();
        for anchor in anchors {
            grouped
                .entry(anchor.file_path.clone())
                .or_insert_with(Vec::new)
                .push(anchor);
        }
        Ok(grouped)
    }

    pub fn note_for_anchor(&self, anchor: &AnchorRecord) -> Result<Option<NodeRecord>> {
        let anchors = self.anchors_in_file(&anchor.file_path)?;
        Ok(note_for_anchor_in_file(&anchors, &anchor.node_key))
    }

    pub(crate) fn note_owners_by_anchor_key(
        &self,
        file_path: &str,
    ) -> Result<HashMap<String, NodeRecord>> {
        let anchors = self.anchors_in_file(file_path)?;
        Ok(note_owners_by_anchor_key(&anchors))
    }
}

pub(crate) fn anchor_select_columns(alias: &str) -> String {
    format!(
        "{alias}.node_key,
         {alias}.explicit_id,
         {alias}.file_path,
         {alias}.title,
         {alias}.outline_path,
         {alias}.aliases_json,
         {alias}.tags_json,
         {alias}.refs_json,
         {alias}.todo_keyword,
         {alias}.scheduled_for,
         {alias}.deadline_for,
         {alias}.closed_at,
         {alias}.level,
         {alias}.line,
         {alias}.kind,
         COALESCE((SELECT f.mtime_ns
                     FROM files AS f
                    WHERE f.path = {alias}.file_path), 0) AS file_mtime_ns,
         COALESCE((SELECT COUNT(*)
                     FROM links AS incoming
                    WHERE incoming.destination_explicit_id = {alias}.explicit_id), 0) AS backlink_count,
         COALESCE((SELECT COUNT(*)
                     FROM links AS outgoing
                     JOIN nodes AS dest ON dest.explicit_id = outgoing.destination_explicit_id
                    WHERE outgoing.source_node_key = {alias}.node_key), 0) AS forward_link_count"
    )
}

pub(crate) fn note_where(alias: &str) -> String {
    format!("({alias}.kind = 'file' OR {alias}.explicit_id IS NOT NULL)")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PointLookupAnchor {
    node_key: String,
    explicit_id: Option<String>,
    kind: NodeKind,
    level: u32,
}

impl PointLookupAnchor {
    #[must_use]
    fn is_note(&self) -> bool {
        matches!(self.kind, NodeKind::File) || self.explicit_id.is_some()
    }
}

fn search_nodes_order_by(sort: Option<&SearchNodesSort>, using_fts: bool) -> &'static str {
    match sort {
        None | Some(SearchNodesSort::Relevance) if using_fts => {
            "bm25(node_fts, 1.0, 0.3, 0.2, 0.7, 0.8, 0.4), n.file_path, n.line"
        }
        None | Some(SearchNodesSort::Relevance) => "n.file_path, n.line",
        Some(SearchNodesSort::Title) => "n.title COLLATE NOCASE, n.file_path, n.line",
        Some(SearchNodesSort::File) => "n.file_path, n.line",
        Some(SearchNodesSort::FileMtime) => "file_mtime_ns DESC, n.file_path, n.line",
        Some(SearchNodesSort::BacklinkCount) => "backlink_count DESC, n.file_path, n.line",
        Some(SearchNodesSort::ForwardLinkCount) => "forward_link_count DESC, n.file_path, n.line",
    }
}

pub(crate) fn row_to_anchor(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnchorRecord> {
    let kind_text: String = row.get(14)?;
    Ok(AnchorRecord {
        node_key: row.get(0)?,
        explicit_id: row.get(1)?,
        file_path: row.get(2)?,
        title: row.get(3)?,
        outline_path: row.get(4)?,
        aliases: parse_string_list(row.get::<_, String>(5)?),
        tags: parse_string_list(row.get::<_, String>(6)?),
        refs: parse_string_list(row.get::<_, String>(7)?),
        todo_keyword: row.get(8)?,
        scheduled_for: row.get(9)?,
        deadline_for: row.get(10)?,
        closed_at: row.get(11)?,
        level: row.get(12)?,
        line: row.get(13)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
        file_mtime_ns: row.get(15)?,
        backlink_count: row.get(16)?,
        forward_link_count: row.get(17)?,
    })
}

fn row_to_point_lookup_anchor(row: &rusqlite::Row<'_>) -> rusqlite::Result<PointLookupAnchor> {
    let kind_text: String = row.get(2)?;
    Ok(PointLookupAnchor {
        node_key: row.get(0)?,
        explicit_id: row.get(1)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
        level: row.get(3)?,
    })
}

pub(crate) fn row_to_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<NodeRecord> {
    let anchor = row_to_anchor(row)?;
    NodeRecord::try_from(anchor).map_err(|anchor| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(format!(
                "anchor {} is not a canonical note",
                anchor.node_key
            ))),
        )
    })
}

pub(crate) fn row_to_anchor_with_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<AnchorRecord> {
    let kind_text: String = row.get(offset + 14)?;
    Ok(AnchorRecord {
        node_key: row.get(offset)?,
        explicit_id: row.get(offset + 1)?,
        file_path: row.get(offset + 2)?,
        title: row.get(offset + 3)?,
        outline_path: row.get(offset + 4)?,
        aliases: parse_string_list(row.get::<_, String>(offset + 5)?),
        tags: parse_string_list(row.get::<_, String>(offset + 6)?),
        refs: parse_string_list(row.get::<_, String>(offset + 7)?),
        todo_keyword: row.get(offset + 8)?,
        scheduled_for: row.get(offset + 9)?,
        deadline_for: row.get(offset + 10)?,
        closed_at: row.get(offset + 11)?,
        level: row.get(offset + 12)?,
        line: row.get(offset + 13)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
        file_mtime_ns: row.get(offset + 15)?,
        backlink_count: row.get(offset + 16)?,
        forward_link_count: row.get(offset + 17)?,
    })
}

pub(crate) fn row_to_note_with_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<NodeRecord> {
    let anchor = row_to_anchor_with_offset(row, offset)?;
    NodeRecord::try_from(anchor).map_err(|anchor| {
        rusqlite::Error::FromSqlConversionFailure(
            offset,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::other(format!(
                "anchor {} is not a canonical note",
                anchor.node_key
            ))),
        )
    })
}

fn note_for_anchor_in_file(anchors: &[AnchorRecord], anchor_key: &str) -> Option<NodeRecord> {
    note_owners_by_anchor_key(anchors).remove(anchor_key)
}

pub(crate) fn note_owners_by_anchor_key(anchors: &[AnchorRecord]) -> HashMap<String, NodeRecord> {
    let anchor_lookup = anchors
        .iter()
        .map(|anchor| (anchor.node_key.as_str(), anchor))
        .collect::<HashMap<_, _>>();
    let owner_keys = anchor_owner_note_keys(anchors);
    owner_keys
        .into_iter()
        .filter_map(|(anchor_key, owner_key)| {
            let owner = anchor_lookup.get(owner_key.as_str())?;
            let note = NodeRecord::try_from((*owner).clone()).ok()?;
            Some((anchor_key, note))
        })
        .collect()
}

fn anchor_owner_note_keys(anchors: &[AnchorRecord]) -> HashMap<String, String> {
    let mut owner_keys = HashMap::new();
    let mut ancestry: Vec<&AnchorRecord> = Vec::new();
    let mut note_stack: Vec<&AnchorRecord> = Vec::new();

    for anchor in anchors {
        while let Some(last) = ancestry.last().copied() {
            if matches!(last.kind, NodeKind::File) && !matches!(anchor.kind, NodeKind::File) {
                break;
            }
            if last.level < anchor.level {
                break;
            }
            ancestry.pop();
            if note_stack
                .last()
                .is_some_and(|candidate| candidate.node_key == last.node_key)
            {
                note_stack.pop();
            }
        }

        let owner_key = if anchor.is_note() {
            anchor.node_key.clone()
        } else {
            note_stack
                .last()
                .map(|candidate| candidate.node_key.clone())
                .unwrap_or_else(|| anchor.node_key.clone())
        };
        owner_keys.insert(anchor.node_key.clone(), owner_key);

        ancestry.push(anchor);
        if anchor.is_note() {
            note_stack.push(anchor);
        }
    }

    owner_keys
}

fn build_fts_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .filter_map(|term| {
            let trimmed = term.trim_matches(|character: char| !character.is_alphanumeric());
            if trimmed.len() >= 3 {
                Some(trimmed)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if terms.is_empty() {
        None
    } else {
        Some(
            terms
                .into_iter()
                .map(|term| format!("\"{}\"*", term.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

fn escape_like_pattern(input: &str) -> String {
    let mut escaped = String::new();
    for character in input.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn parse_string_list(value: String) -> Vec<String> {
    match serde_json::from_str::<Value>(&value) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

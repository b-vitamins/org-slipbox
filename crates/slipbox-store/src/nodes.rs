use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde_json::Value;

use slipbox_core::{
    AnchorRecord, ContentSegment, ContentSnippet, MIN_SEARCH_TERM_CHARACTERS, NodeContentHit,
    NodeKind, NodeRecord, SearchNodesSort,
};

use crate::Database;

pub(crate) const ANCHOR_SELECT_COLUMN_COUNT: usize = 25;

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

    pub fn list_glossary_terms(&self, limit: usize) -> Result<Vec<NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
              ORDER BY n.title COLLATE NOCASE, n.file_path, n.line
              LIMIT ?1",
            anchor_select_columns("n"),
            glossary_where("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit.clamp(1, 200) as i64], row_to_note)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read glossary terms")
    }

    pub fn search_glossary(&self, query: &str, limit: usize) -> Result<Vec<NodeRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if let Some(fts_query) = build_fts_query(query) {
            let sql = format!(
                "SELECT {}
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                    AND {}
                  ORDER BY bm25(node_fts, 1.0, 0.3, 0.2, 0.7, 0.8, 0.4), n.file_path, n.line
                  LIMIT ?2",
                anchor_select_columns("n"),
                glossary_where("n"),
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![fts_query, limit], row_to_note)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read glossary search results")
        } else {
            // A query with no usable FTS terms lists the glossary the same way
            // `list_glossary_terms` does, mirroring the note search fallback.
            let sql = format!(
                "SELECT {}
                   FROM nodes AS n
                  WHERE {}
                  ORDER BY n.title COLLATE NOCASE, n.file_path, n.line
                  LIMIT ?1",
                anchor_select_columns("n"),
                glossary_where("n"),
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![limit], row_to_note)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read glossary listing")
        }
    }

    /// Search note body, title, and aliases through `node_content_fts`, ranked,
    /// with a highlighted snippet per hit.
    pub fn search_node_content(&self, query: &str, limit: usize) -> Result<Vec<NodeContentHit>> {
        let limit = limit.clamp(1, 200) as i64;
        let Some(fts_query) = build_fts_query(query) else {
            return Ok(Vec::new());
        };
        // The phrase probe runs against `node_phrase_fts`, which is unstemmed and
        // asks for exact terms; `node_content_fts` stems with `porter`, so a phrase
        // there would match a merely related spelling. The probe is joined in, never
        // filtered on.
        let phrase_query = build_fts_phrase_query(query);
        let (phrase_join, phrase_rank) = match phrase_query {
            Some(_) => (
                "LEFT JOIN (SELECT rowid
                              FROM node_phrase_fts
                             WHERE node_phrase_fts MATCH ?5) AS phrase
                        ON phrase.rowid = node_content_fts.rowid",
                "phrase.rowid IS NULL, ",
            ),
            None => ("", ""),
        };
        // The two equality tests below are the ones `idx_nodes_title_nocase` and
        // `idx_aliases_alias_nocase` serve. `UNION` keeps them as separate indexed
        // lookups; a single `OR` bridging `nodes` and `aliases` would reach neither
        // index. NOCASE is ASCII only and does not reach the diacritics the FTS
        // tokenizers strip.
        let headword_query = query.split_whitespace().collect::<Vec<_>>().join(" ");
        // The snippet comes from the body column (index 2), wrapping matched runs in
        // control characters that cannot occur in Org prose.
        let sql = format!(
            "SELECT {},
                    snippet(node_content_fts, 2, char(2), char(3), '…', ?2)
               FROM node_content_fts
               JOIN nodes AS n ON n.id = node_content_fts.rowid
               LEFT JOIN (SELECT id
                            FROM nodes
                           WHERE title = ?4 COLLATE NOCASE
                           UNION
                          SELECT nodes.id
                            FROM nodes
                            JOIN aliases ON aliases.node_key = nodes.node_key
                           WHERE aliases.alias = ?4 COLLATE NOCASE) AS headword
                       ON headword.id = node_content_fts.rowid
               {}
              WHERE node_content_fts MATCH ?1
                AND {}
              ORDER BY headword.id IS NULL, {}bm25(node_content_fts), n.file_path, n.line
              LIMIT ?3",
            anchor_select_columns("n"),
            phrase_join,
            note_where("n"),
            phrase_rank,
        );
        // The phrase param is bound last so parameter numbering holds whether or not
        // the query has a phrase to prefer.
        let mut arguments: Vec<rusqlite::types::Value> = vec![
            fts_query.into(),
            SNIPPET_TOKEN_BUDGET.into(),
            limit.into(),
            headword_query.into(),
        ];
        arguments.extend(phrase_query.map(Into::into));
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(arguments), |row| {
            let node = row_to_note(row)?;
            let raw: String = row.get(ANCHOR_SELECT_COLUMN_COUNT)?;
            Ok(NodeContentHit {
                node,
                snippet: parse_snippet(&raw),
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read note content search results")
    }

    pub fn glossary_due_terms(&self, today: &str, limit: usize) -> Result<Vec<NodeRecord>> {
        // A term is due when it has never been reviewed (`reps` unset or zero) or
        // its stored due date is at or before `today`. Scheduling values are
        // mirrored verbatim as text, so `CAST` matches the core scheduler, which
        // parses a missing or malformed `reps` back to zero.
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
                AND (COALESCE(CAST(n.sr_reps AS INTEGER), 0) = 0
                     OR n.sr_due IS NULL
                     OR n.sr_due <= ?1)
              ORDER BY n.sr_due, n.file_path, n.line
              LIMIT ?2",
            anchor_select_columns("n"),
            glossary_where("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![today, limit.clamp(1, 200) as i64], row_to_note)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read due glossary terms")
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
         {alias}.backlink_count,
         {alias}.forward_link_count,
         {alias}.glossary,
         {alias}.glossary_status,
         {alias}.sr_due,
         {alias}.sr_ease,
         {alias}.sr_interval,
         {alias}.sr_reps,
         {alias}.sr_last"
    )
}

pub(crate) fn note_where(alias: &str) -> String {
    format!("({alias}.kind = 'file' OR {alias}.explicit_id IS NOT NULL)")
}

fn glossary_where(alias: &str) -> String {
    // A glossary term is a marked note, so the row must both carry the marker and
    // hydrate through `row_to_note` as a canonical note.
    format!("{alias}.glossary = 1 AND {}", note_where(alias))
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
        glossary: row.get::<_, i64>(18)? != 0,
        glossary_status: row.get(19)?,
        sr_due: row.get(20)?,
        sr_ease: row.get(21)?,
        sr_interval: row.get(22)?,
        sr_reps: row.get(23)?,
        sr_last: row.get(24)?,
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
        glossary: row.get::<_, i64>(offset + 18)? != 0,
        glossary_status: row.get(offset + 19)?,
        sr_due: row.get(offset + 20)?,
        sr_ease: row.get(offset + 21)?,
        sr_interval: row.get(offset + 22)?,
        sr_reps: row.get(offset + 23)?,
        sr_last: row.get(offset + 24)?,
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

/// Maximum tokens FTS5 puts in a content snippet before eliding.
const SNIPPET_TOKEN_BUDGET: i64 = 32;

/// Start-of-match delimiter, a C0 control character that never appears in Org
/// prose.
const MATCH_OPEN: char = '\u{2}';

/// End-of-match delimiter paired with [`MATCH_OPEN`].
const MATCH_CLOSE: char = '\u{3}';

/// Split an FTS5 `snippet()` string on the match delimiters into alternating
/// plain and matched segments, dropping the delimiters and any empty run.
fn parse_snippet(raw: &str) -> ContentSnippet {
    fn flush(text: &mut String, matched: bool, segments: &mut Vec<ContentSegment>) {
        if !text.is_empty() {
            segments.push(ContentSegment {
                text: std::mem::take(text),
                matched,
            });
        }
    }

    let mut segments = Vec::new();
    let mut matched = false;
    let mut current = String::new();

    for character in raw.chars() {
        match character {
            MATCH_OPEN => {
                flush(&mut current, matched, &mut segments);
                matched = true;
            }
            MATCH_CLOSE => {
                flush(&mut current, matched, &mut segments);
                matched = false;
            }
            _ => current.push(character),
        }
    }
    flush(&mut current, matched, &mut segments);

    ContentSnippet { segments }
}

/// Usable FTS terms in a raw query: whitespace-separated runs, stripped of
/// surrounding punctuation, at least [`MIN_SEARCH_TERM_CHARACTERS`] long.
fn fts_terms(query: &str) -> Vec<&str> {
    query
        .split_whitespace()
        .filter_map(|term| {
            let trimmed = term.trim_matches(|character: char| !character.is_alphanumeric());
            if trimmed.chars().count() >= MIN_SEARCH_TERM_CHARACTERS {
                Some(trimmed)
            } else {
                None
            }
        })
        .collect()
}

/// Render terms as quoted FTS5 terms joined by `separator`, each a prefix term
/// when `prefix` is set.
fn join_fts_terms(terms: &[&str], separator: &str, prefix: bool) -> String {
    let star = if prefix { "*" } else { "" };
    terms
        .iter()
        .map(|term| format!("\"{}\"{star}", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(separator)
}

fn build_fts_query(query: &str) -> Option<String> {
    let terms = fts_terms(query);
    if terms.is_empty() {
        None
    } else {
        Some(join_fts_terms(&terms, " ", true))
    }
}

/// The query terms as one FTS5 phrase joined by `+`, whole rather than prefixes,
/// for the unstemmed `node_phrase_fts`. A single term yields `None`.
fn build_fts_phrase_query(query: &str) -> Option<String> {
    let terms = fts_terms(query);
    if terms.len() < 2 {
        None
    } else {
        Some(join_fts_terms(&terms, " + ", false))
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};

    use crate::test_support::indexed_database;

    fn titles(terms: &[slipbox_core::NodeRecord]) -> Vec<String> {
        terms.iter().map(|term| term.title.clone()).collect()
    }

    fn term(title: &str, extra_drawer: &str, body: &str) -> String {
        format!("#+title: {title}\n#+glossary: t\n:PROPERTIES:\n{extra_drawer}:END:\n\n{body}\n")
    }

    #[test]
    fn list_returns_only_marked_terms() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("integral.org", &term("Integral", "", "Area under a curve.")),
            (
                "derivative.org",
                &term("Derivative", "", "Instantaneous rate of change."),
            ),
            ("plain.org", "#+title: Plain note\n\nNot a term.\n"),
        ])?;

        let terms = database.list_glossary_terms(50)?;
        assert_eq!(titles(&terms), vec!["Derivative", "Integral"]);
        Ok(())
    }

    #[test]
    fn list_respects_limit() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("a.org", &term("Alpha", "", "First.")),
            ("b.org", &term("Beta", "", "Second.")),
            ("c.org", &term("Gamma", "", "Third.")),
        ])?;

        let terms = database.list_glossary_terms(2)?;
        assert_eq!(titles(&terms), vec!["Alpha", "Beta"]);
        Ok(())
    }

    #[test]
    fn search_returns_only_marked_terms() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "integral.org",
                &term("Riemann integral", "", "Limit of Riemann sums."),
            ),
            (
                "sums.org",
                "#+title: Riemann sums\n\nAn ordinary note about sums.\n",
            ),
        ])?;

        let terms = database.search_glossary("Riemann", 20)?;
        assert_eq!(titles(&terms), vec!["Riemann integral"]);
        Ok(())
    }

    #[test]
    fn search_stems_and_folds_like_note_search() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("d.org", &term("Derivative", "", "Rate of change.")),
            ("g.org", &term("Gödel", "", "Incompleteness.")),
        ])?;

        assert_eq!(
            titles(&database.search_glossary("derivatives", 20)?),
            vec!["Derivative"]
        );
        assert_eq!(
            titles(&database.search_glossary("Godel", 20)?),
            vec!["Gödel"]
        );
        Ok(())
    }

    #[test]
    fn search_with_no_terms_lists_glossary() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("b.org", &term("Beta", "", "Second.")),
            ("a.org", &term("Alpha", "", "First.")),
            ("plain.org", "#+title: Plain\n\nNot a term.\n"),
        ])?;

        // A punctuation-only query has no usable FTS terms and falls back to a
        // title-ordered listing of marked terms only.
        let terms = database.search_glossary("!!", 20)?;
        assert_eq!(titles(&terms), vec!["Alpha", "Beta"]);
        Ok(())
    }

    #[test]
    fn due_includes_unreviewed_and_overdue_terms() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            // Never reviewed: due regardless of any missing schedule.
            ("new.org", &term("New", "", "Fresh card.")),
            // Overdue: due date before today.
            (
                "overdue.org",
                &term(
                    "Overdue",
                    ":SR_DUE:  2026-07-01\n:SR_REPS: 3\n",
                    "Was due earlier.",
                ),
            ),
            // Due exactly today.
            (
                "today.org",
                &term("Today", ":SR_DUE:  2026-07-21\n:SR_REPS: 2\n", "Due today."),
            ),
            // Scheduled ahead: not due yet.
            (
                "future.org",
                &term(
                    "Future",
                    ":SR_DUE:  2026-08-01\n:SR_REPS: 4\n",
                    "Not due yet.",
                ),
            ),
        ])?;

        let due = database.glossary_due_terms("2026-07-21", 50)?;
        let mut due_titles = titles(&due);
        due_titles.sort();
        assert_eq!(due_titles, vec!["New", "Overdue", "Today"]);
        Ok(())
    }

    #[test]
    fn due_orders_by_due_date_and_respects_limit() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "a.org",
                &term("A", ":SR_DUE:  2026-07-10\n:SR_REPS: 1\n", "Earliest."),
            ),
            (
                "b.org",
                &term("B", ":SR_DUE:  2026-07-15\n:SR_REPS: 1\n", "Middle."),
            ),
            (
                "c.org",
                &term("C", ":SR_DUE:  2026-07-20\n:SR_REPS: 1\n", "Latest."),
            ),
        ])?;

        let due = database.glossary_due_terms("2026-07-21", 2)?;
        assert_eq!(titles(&due), vec!["A", "B"]);
        Ok(())
    }

    fn content_titles(hits: &[slipbox_core::NodeContentHit]) -> Vec<String> {
        hits.iter().map(|hit| hit.node.title.clone()).collect()
    }

    fn snippet_text(hit: &slipbox_core::NodeContentHit) -> String {
        hit.snippet
            .segments
            .iter()
            .map(|segment| segment.text.as_str())
            .collect()
    }

    fn matched_text(hit: &slipbox_core::NodeContentHit) -> Vec<String> {
        hit.snippet
            .segments
            .iter()
            .filter(|segment| segment.matched)
            .map(|segment| segment.text.clone())
            .collect()
    }

    #[test]
    fn content_search_matches_body_and_highlights_the_term() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "integral.org",
            "#+title: Integral\n\nThe fundamental theorem links it to the derivative.\n",
        )])?;

        let hits = database.search_node_content("fundamental", 20)?;
        assert_eq!(content_titles(&hits), vec!["Integral"]);
        assert!(
            snippet_text(&hits[0]).contains("fundamental theorem"),
            "the snippet should quote the matched body prose"
        );
        assert_eq!(
            matched_text(&hits[0]),
            vec!["fundamental"],
            "only the query term is highlighted"
        );
        Ok(())
    }

    #[test]
    fn content_search_reaches_title_and_aliases() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "d.org",
            "#+title: Derivative\n:PROPERTIES:\n:ROAM_ALIASES: \"Differential quotient\"\n:END:\n\nRate of change.\n",
        )])?;

        assert_eq!(
            content_titles(&database.search_node_content("Derivative", 20)?),
            vec!["Derivative"]
        );
        assert_eq!(
            content_titles(&database.search_node_content("quotient", 20)?),
            vec!["Derivative"]
        );
        Ok(())
    }

    #[test]
    fn content_search_stems_and_folds_like_note_search() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "d.org",
                "#+title: Rates\n\nThe derivative measures instantaneous change.\n",
            ),
            ("g.org", "#+title: Logic\n\nGödel proved incompleteness.\n"),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("derivatives", 20)?),
            vec!["Rates"]
        );
        assert_eq!(
            content_titles(&database.search_node_content("godel", 20)?),
            vec!["Logic"]
        );
        Ok(())
    }

    fn phrase_fixture() -> [(&'static str, &'static str); 2] {
        [
            (
                "local.org",
                "#+title: Local optimality and the maximum principle\n\nMaximization is global here.\n",
            ),
            (
                "proto.org",
                "#+title: Proto-maximum principle\n\nThe proto rule anticipates global maximization by a century, and the surrounding discussion works through compactness, continuity, convexity, and a long list of side conditions before it reaches any conclusion at all.\n",
            ),
        ]
    }

    #[test]
    fn content_search_ranks_the_exact_phrase_first() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&phrase_fixture())?;

        assert_eq!(
            content_titles(&database.search_node_content("global maximization", 20)?),
            vec![
                "Proto-maximum principle",
                "Local optimality and the maximum principle"
            ],
            "the note spelling the phrase out ranks above one merely mentioning both words"
        );
        Ok(())
    }

    #[test]
    fn content_search_does_not_read_a_sentence_boundary_as_a_phrase() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "decoy.org",
                "#+title: Decoy\n\nEvery local minimum is automatically global. Maxima are defined by reversing the sign.\n",
            ),
            (
                "phrase.org",
                "#+title: Phrase\n\nThe variational approach establishes stationarity, not global maximization, and the surrounding discussion works through compactness, continuity, convexity, and a long list of side conditions before it reaches any conclusion at all.\n",
            ),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("global maximization", 20)?),
            vec!["Phrase", "Decoy"]
        );
        Ok(())
    }

    #[test]
    fn content_search_does_not_read_a_shared_stem_as_a_phrase() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "stemmed.org",
                "#+title: Stemmed\n\nGlobal maximizing behaviour.\n",
            ),
            (
                "spelled.org",
                "#+title: Spelled\n\nGlobal maximization holds here, and the surrounding discussion works through compactness, continuity, convexity, and a long list of side conditions before it reaches any conclusion at all.\n",
            ),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("global maximization", 20)?),
            vec!["Spelled", "Stemmed"]
        );
        Ok(())
    }

    #[test]
    fn content_search_keeps_every_multi_word_match() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&phrase_fixture())?;

        let mut both = content_titles(&database.search_node_content("global maximization", 20)?);
        both.sort();
        assert_eq!(
            both,
            vec![
                "Local optimality and the maximum principle",
                "Proto-maximum principle"
            ]
        );
        assert_eq!(database.search_node_content("global", 20)?.len(), 2);
        assert_eq!(database.search_node_content("maximization", 20)?.len(), 2);
        Ok(())
    }

    #[test]
    fn content_search_ranks_a_single_word_query_by_relevance_alone() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "long.org",
                "#+title: Long\n\nGlobal maximization opens a long discussion that wanders through compactness, continuity, convexity, and several other conditions before it ever reaches a conclusion.\n",
            ),
            ("short.org", "#+title: Short\n\nMaximization.\n"),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("maximization", 20)?),
            vec!["Short", "Long"]
        );
        Ok(())
    }

    #[test]
    fn content_search_still_matches_words_in_a_different_order() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&phrase_fixture())?;

        let mut reversed =
            content_titles(&database.search_node_content("maximization global", 20)?);
        reversed.sort();
        assert_eq!(
            reversed,
            vec![
                "Local optimality and the maximum principle",
                "Proto-maximum principle"
            ]
        );
        Ok(())
    }

    #[test]
    fn content_search_phrase_preference_folds_case_and_whitespace() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&phrase_fixture())?;

        let expected = vec![
            "Proto-maximum principle".to_owned(),
            "Local optimality and the maximum principle".to_owned(),
        ];
        assert_eq!(
            content_titles(&database.search_node_content("  GLOBAL   Maximization ", 20)?),
            expected
        );
        Ok(())
    }

    fn headword_fixture() -> [(&'static str, &'static str); 3] {
        [
            (
                "hamiltonian.org",
                "#+title: Hamiltonian\n\nThe total energy of a system, written in canonical coordinates.\n",
            ),
            (
                "pmp.org",
                "#+title: Time-dependent Hamiltonian in the PMP\n\nThe Hamiltonian of a control problem varies with time, so the Hamiltonian is no longer conserved along an extremal; the maximized Hamiltonian picks up the partial derivative of the Hamiltonian with respect to time, and the transversality condition on the Hamiltonian closes the system.\n",
            ),
            (
                "legendre.org",
                "#+title: Legendre transform\n\nIt carries a Lagrangian to a Hamiltonian.\n",
            ),
        ]
    }

    #[test]
    fn content_search_ranks_an_exact_title_first() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&headword_fixture())?;

        assert_eq!(
            content_titles(&database.search_node_content("Hamiltonian", 20)?),
            vec![
                "Hamiltonian",
                "Time-dependent Hamiltonian in the PMP",
                "Legendre transform"
            ],
            "the note the query names outright ranks above every note merely discussing it"
        );
        Ok(())
    }

    #[test]
    fn content_search_exact_title_preference_folds_case() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&headword_fixture())?;

        assert_eq!(
            content_titles(&database.search_node_content("  hAMILTONIAN ", 20)?),
            vec![
                "Hamiltonian",
                "Time-dependent Hamiltonian in the PMP",
                "Legendre transform"
            ]
        );
        Ok(())
    }

    #[test]
    fn content_search_ranks_an_exact_alias_first() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "energy.org",
                "#+title: Total energy function\n:PROPERTIES:\n:ROAM_ALIASES: Hamiltonian\n:END:\n\nWritten in canonical coordinates.\n",
            ),
            headword_fixture()[1],
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("Hamiltonian", 20)?),
            vec![
                "Total energy function",
                "Time-dependent Hamiltonian in the PMP"
            ]
        );
        Ok(())
    }

    #[test]
    fn content_search_keeps_every_match_around_an_exact_title() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&headword_fixture())?;

        let mut every = content_titles(&database.search_node_content("Hamiltonian", 20)?);
        every.sort();
        assert_eq!(
            every,
            vec![
                "Hamiltonian",
                "Legendre transform",
                "Time-dependent Hamiltonian in the PMP"
            ]
        );
        Ok(())
    }

    #[test]
    fn content_search_ranks_an_exact_multi_word_title_first() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "minima.org",
                "#+title: local and global minima\n\nA short gloss on the distinction.\n",
            ),
            (
                "descent.org",
                "#+title: Descent methods\n\nThe distinction between local and global minima drives the subject: local and global minima coincide for a convex objective, while for a non-convex one the gap between local and global minima is what every escape heuristic chases.\n",
            ),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("local and global minima", 20)?),
            vec!["local and global minima", "Descent methods"]
        );
        Ok(())
    }

    #[test]
    fn content_search_prefers_the_phrase_among_untitled_matches() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&phrase_fixture())?;

        assert_eq!(
            content_titles(&database.search_node_content("global maximization", 20)?),
            vec![
                "Proto-maximum principle",
                "Local optimality and the maximum principle"
            ]
        );
        Ok(())
    }

    #[test]
    fn a_two_character_acronym_is_a_term_every_search_path_matches_on() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "kl.org",
                "#+title: KL divergence\n\nAn asymmetric measure between two distributions.\n",
            ),
            (
                "descent.org",
                "#+title: Gradient descent\n\nA note that has nothing to do with it.\n",
            ),
        ])?;

        assert_eq!(
            titles(&database.search_nodes("KL", 20, None)?),
            vec!["KL divergence"]
        );
        assert_eq!(
            content_titles(&database.search_node_content("KL", 20)?),
            vec!["KL divergence"]
        );
        Ok(())
    }

    #[test]
    fn a_word_below_the_floor_is_not_a_term_whatever_its_byte_length() -> Result<()> {
        let (_workspace, database, _root) =
            indexed_database(&[("cat.org", "#+title: 猫\n\nA single-character headword.\n")])?;

        assert!(database.search_node_content("猫", 20)?.is_empty());
        assert!(database.search_node_content("a", 20)?.is_empty());
        Ok(())
    }

    #[test]
    fn content_search_scopes_snippet_to_the_owning_node() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "doc.org",
            "#+title: Parent\n\nParent mentions apples.\n\n* Child\n:PROPERTIES:\n:ID: child1\n:END:\n\nChild mentions oranges.\n",
        )])?;

        let apple = database.search_node_content("apples", 20)?;
        assert_eq!(content_titles(&apple), vec!["Parent"]);

        let orange = database.search_node_content("oranges", 20)?;
        assert_eq!(content_titles(&orange), vec!["Child"]);
        assert!(
            !snippet_text(&orange[0]).contains("apples"),
            "the child's snippet must not leak the parent's prose"
        );
        Ok(())
    }

    #[test]
    fn content_search_with_no_usable_terms_returns_nothing() -> Result<()> {
        let (_workspace, database, _root) =
            indexed_database(&[("a.org", "#+title: Alpha\n\nBody text.\n")])?;

        assert!(database.search_node_content("!!", 20)?.is_empty());
        Ok(())
    }

    #[test]
    fn content_snippet_never_reinterprets_markup_in_prose() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "m.org",
            "#+title: Markup\n\nThe unmistakable widget renders <mark> and [[id:abc][a link]].\n",
        )])?;

        let hits = database.search_node_content("unmistakable", 20)?;
        assert_eq!(content_titles(&hits), vec!["Markup"]);
        let plain: String = hits[0]
            .snippet
            .segments
            .iter()
            .filter(|segment| !segment.matched)
            .map(|segment| segment.text.as_str())
            .collect();
        assert!(plain.contains("<mark>"));
        assert!(plain.contains("[[id:abc][a link]]"));
        assert_eq!(matched_text(&hits[0]), vec!["unmistakable"]);
        Ok(())
    }

    #[test]
    fn content_search_leaves_metadata_search_recall_unchanged() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "n.org",
            "#+title: Topology\n\nThe unmistakable body keyword lives here.\n",
        )])?;

        assert!(
            database.search_nodes("unmistakable", 20, None)?.is_empty(),
            "a body-only word must not surface through node_fts metadata search"
        );
        assert_eq!(
            content_titles(&database.search_node_content("unmistakable", 20)?),
            vec!["Topology"]
        );
        Ok(())
    }

    #[test]
    fn content_index_survives_forced_rebuild() -> Result<()> {
        let (_workspace, mut database, root) = indexed_database(&[(
            "term.org",
            "#+title: Integral\n\nThe fundamental theorem of calculus.\n",
        )])?;

        assert_eq!(
            content_titles(&database.search_node_content("fundamental", 20)?),
            vec!["Integral"]
        );

        database
            .connection
            .execute_batch("PRAGMA user_version = 0;")?;
        database.migrate()?;
        assert!(
            database.search_node_content("fundamental", 20)?.is_empty(),
            "a forced rebuild empties the content index"
        );

        let files = scan_root_with_policy(&root, &DiscoveryPolicy::default())?;
        database.sync_index(&files)?;
        assert_eq!(
            content_titles(&database.search_node_content("fundamental", 20)?),
            vec!["Integral"]
        );
        Ok(())
    }

    #[test]
    fn glossary_state_survives_forced_rebuild() -> Result<()> {
        let (_workspace, mut database, root) = indexed_database(&[(
            "term.org",
            &term(
                "Riemann integral",
                ":GLOSSARY_STATUS: confirmed\n:SR_DUE:      2026-08-01\n:SR_EASE:     2.50\n:SR_INTERVAL: 6\n:SR_REPS:     3\n:SR_LAST:     2026-07-26\n",
                "Limit of Riemann sums.",
            ),
        )])?;

        let before = database
            .list_glossary_terms(10)?
            .into_iter()
            .next()
            .expect("the marked term is indexed");
        assert!(before.glossary);
        assert_eq!(before.glossary_status.as_deref(), Some("confirmed"));
        assert_eq!(before.sr_due.as_deref(), Some("2026-08-01"));
        assert_eq!(before.sr_reps.as_deref(), Some("3"));

        // Force a schema rebuild: the derived tables are dropped and re-created
        // empty on the next open, proving nothing lives only in SQLite.
        database
            .connection
            .execute_batch("PRAGMA user_version = 0;")?;
        database.migrate()?;
        assert!(
            database.list_glossary_terms(10)?.is_empty(),
            "a forced rebuild empties the derived index"
        );

        // Re-sync from Org (the source of truth) repopulates every column.
        let files = scan_root_with_policy(&root, &DiscoveryPolicy::default())?;
        database.sync_index(&files)?;

        let after = database
            .list_glossary_terms(10)?
            .into_iter()
            .next()
            .expect("the term reappears after re-sync");
        assert_eq!(after.glossary, before.glossary);
        assert_eq!(after.glossary_status, before.glossary_status);
        assert_eq!(after.sr_due, before.sr_due);
        assert_eq!(after.sr_ease, before.sr_ease);
        assert_eq!(after.sr_interval, before.sr_interval);
        assert_eq!(after.sr_reps, before.sr_reps);
        assert_eq!(after.sr_last, before.sr_last);
        Ok(())
    }
}

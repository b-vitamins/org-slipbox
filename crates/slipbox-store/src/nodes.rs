use std::collections::HashMap;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde_json::Value;

use slipbox_core::{
    AnchorRecord, ContentSegment, ContentSnippet, MIN_SEARCH_TERM_CHARACTERS, NodeContentHit,
    NodeKind, NodeRecord, NotePlaceNeighbor, NotePlaceResult, SearchNodesSort,
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
            let literal = relevance_literal_probe(query, sort.as_ref());
            let sql = search_nodes_fts_sql(sort.as_ref(), Some(&note_where), literal.is_some());
            let mut arguments: Vec<rusqlite::types::Value> = vec![fts_query.into(), limit.into()];
            arguments.extend(literal.map(Into::into));
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(arguments), row_to_note)?;
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
            let literal = relevance_literal_probe(query, sort.as_ref());
            let sql = search_nodes_fts_sql(sort.as_ref(), None, literal.is_some());
            let mut arguments: Vec<rusqlite::types::Value> = vec![fts_query.into(), limit.into()];
            arguments.extend(literal.map(Into::into));
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params_from_iter(arguments), row_to_anchor)?;
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

    /// Return a note's 1-based place and immediate filing neighbors.
    pub fn note_place(&self, node_key: &str) -> Result<Option<NotePlaceResult>> {
        let sql = format!(
            "SELECT n.file_path, n.line
               FROM nodes AS n
              WHERE n.node_key = ?1
                AND {}",
            note_where("n"),
        );
        let Some((file_path, line)) = self
            .connection
            .query_row(&sql, params![node_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
            })
            .optional()
            .context("failed to fetch the filed position of a note")?
        else {
            return Ok(None);
        };

        Ok(Some(NotePlaceResult {
            ordinal: self.notes_filed_before(&file_path, line)? + 1,
            total: self.notes_indexed()?,
            earlier: self.filing_neighbor(&file_path, line, FilingSide::Earlier)?,
            later: self.filing_neighbor(&file_path, line, FilingSide::Later)?,
        }))
    }

    fn notes_filed_before(&self, file_path: &str, line: u32) -> Result<u64> {
        self.connection
            .query_row(&notes_filed_before_sql(), params![file_path, line], |row| {
                row.get::<_, u64>(0)
            })
            .context("failed to count the notes filed before a note")
    }

    fn filing_neighbor(
        &self,
        file_path: &str,
        line: u32,
        side: FilingSide,
    ) -> Result<Option<NotePlaceNeighbor>> {
        self.connection
            .query_row(
                &filing_neighbor_sql(side),
                params![file_path, line],
                |row| {
                    Ok(NotePlaceNeighbor {
                        node_key: row.get(0)?,
                        title: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("failed to fetch the note filed beside a note")
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

    /// Return one title-ordered glossary page.
    pub fn list_glossary_terms(
        &self,
        limit: usize,
        after: Option<&GlossaryPosition>,
    ) -> Result<GlossaryPage> {
        let limit = limit.clamp(1, 200);
        let filter = glossary_where("n");
        let total = self.count_glossary(&format!("nodes AS n WHERE {filter}"), [])?;

        let mut arguments: Vec<rusqlite::types::Value> = vec![(limit as i64 + 1).into()];
        let seek = match after {
            None => String::new(),
            Some(position) => {
                arguments.push(position.leading.clone().unwrap_or_default().into());
                arguments.push(position.file_path.clone().into());
                arguments.push(i64::from(position.line).into());
                // The seek and ORDER BY must use the same collation.
                "AND (n.title > ?2 COLLATE NOCASE
                      OR (n.title = ?2 COLLATE NOCASE
                          AND (n.file_path > ?3
                               OR (n.file_path = ?3 AND n.line > ?4))))"
                    .to_owned()
            }
        };
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {filter}
                {seek}
              ORDER BY n.title COLLATE NOCASE, n.file_path, n.line
              LIMIT ?1",
            anchor_select_columns("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(arguments), row_to_note)?;
        let terms = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read glossary terms")?;
        Ok(GlossaryPage::paged(
            terms,
            total,
            limit,
            GlossaryPosition::after_term,
        ))
    }

    /// Return one relevance-ranked glossary search page.
    pub fn search_glossary(&self, query: &str, limit: usize) -> Result<GlossaryPage> {
        let limit = limit.clamp(1, 200) as i64;
        let filter = glossary_where("n");
        if let (Some(fts_query), Some(probes)) =
            (build_fts_query(query), build_fts_literal_probes(query))
        {
            let total = self.count_glossary(
                &format!(
                    "node_fts
                     JOIN nodes AS n ON n.id = node_fts.rowid
                    WHERE node_fts MATCH ?1
                      AND {filter}"
                ),
                params![fts_query],
            )?;
            let sql = search_glossary_sql(&filter);
            let mut statement = self.connection.prepare(&sql)?;
            let rows =
                statement.query_map(params![fts_query, limit, probes.naming], row_to_note)?;
            let terms = rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read glossary search results")?;
            Ok(GlossaryPage::unpaged(terms, total))
        } else {
            // A query with no usable FTS terms lists the glossary the same way
            // `list_glossary_terms` does, mirroring the note search fallback.
            let total = self.count_glossary(&format!("nodes AS n WHERE {filter}"), [])?;
            let sql = format!(
                "SELECT {}
                   FROM nodes AS n
                  WHERE {filter}
                  ORDER BY n.title COLLATE NOCASE, n.file_path, n.line
                  LIMIT ?1",
                anchor_select_columns("n"),
            );
            let mut statement = self.connection.prepare(&sql)?;
            let rows = statement.query_map(params![limit], row_to_note)?;
            let terms = rows
                .collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read glossary listing")?;
            Ok(GlossaryPage::unpaged(terms, total))
        }
    }

    /// Search note body, title, and aliases through `node_content_fts`, ranked,
    /// with a highlighted snippet per hit.
    pub fn search_node_content(&self, query: &str, limit: usize) -> Result<Vec<NodeContentHit>> {
        let limit = limit.clamp(1, 200) as i64;
        let (Some(fts_query), Some(probes)) =
            (build_fts_query(query), build_fts_literal_probes(query))
        else {
            return Ok(Vec::new());
        };
        // Literal probes use the unstemmed index; porter matches related spellings.
        let phrase_query = build_fts_phrase_query(query);
        let headword_query = query.split_whitespace().collect::<Vec<_>>().join(" ");
        let sql = search_node_content_sql(phrase_query.is_some());
        let mut arguments: Vec<rusqlite::types::Value> = vec![
            fts_query.into(),
            SNIPPET_TOKEN_BUDGET.into(),
            limit.into(),
            headword_query.into(),
            probes.naming.into(),
            probes.anywhere.into(),
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

    /// Return one due-order glossary page, optionally filtered by headword or alias.
    pub fn glossary_due_terms(
        &self,
        today: &str,
        query: Option<&str>,
        limit: usize,
        after: Option<&GlossaryPosition>,
    ) -> Result<GlossaryPage> {
        let limit = limit.clamp(1, 200);
        let fts_query = query
            .and_then(build_fts_query)
            .map(|query| format!("{{title alias_text}} : ({query})"));
        let from = if fts_query.is_some() {
            "node_fts JOIN nodes AS n ON n.id = node_fts.rowid"
        } else {
            "nodes AS n"
        };
        let search_filter = if fts_query.is_some() {
            "AND node_fts MATCH ?2"
        } else {
            ""
        };
        let filter = format!(
            "{} AND (COALESCE(CAST(n.sr_reps AS INTEGER), 0) = 0
                     OR n.sr_due IS NULL
                     OR n.sr_due <= ?1)
                {search_filter}",
            glossary_where("n"),
        );
        let mut arguments: Vec<rusqlite::types::Value> = vec![today.to_owned().into()];
        arguments.extend(fts_query.map(Into::into));
        let total = self.count_glossary(
            &format!("{from} WHERE {filter}"),
            params_from_iter(arguments.clone()),
        )?;

        let limit_parameter = arguments.len() + 1;
        arguments.push((limit as i64 + 1).into());
        let seek = match after {
            None => String::new(),
            Some(position) => match &position.leading {
                None => {
                    let path_parameter = arguments.len() + 1;
                    arguments.push(position.file_path.clone().into());
                    let line_parameter = arguments.len() + 1;
                    arguments.push(i64::from(position.line).into());
                    format!(
                        "AND (n.sr_due IS NOT NULL
                              OR n.file_path > ?{path_parameter}
                              OR (n.file_path = ?{path_parameter}
                                  AND n.line > ?{line_parameter}))"
                    )
                }
                Some(due) => {
                    let due_parameter = arguments.len() + 1;
                    arguments.push(due.clone().into());
                    let path_parameter = arguments.len() + 1;
                    arguments.push(position.file_path.clone().into());
                    let line_parameter = arguments.len() + 1;
                    arguments.push(i64::from(position.line).into());
                    format!(
                        "AND (n.sr_due > ?{due_parameter}
                              OR (n.sr_due = ?{due_parameter}
                                  AND (n.file_path > ?{path_parameter}
                                       OR (n.file_path = ?{path_parameter}
                                           AND n.line > ?{line_parameter}))))"
                    )
                }
            },
        };
        let sql = format!(
            "SELECT {}
               FROM {from}
              WHERE {filter}
                {seek}
              ORDER BY n.sr_due, n.file_path, n.line
              LIMIT ?{limit_parameter}",
            anchor_select_columns("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(arguments), row_to_note)?;
        let terms = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read due glossary terms")?;
        Ok(GlossaryPage::paged(
            terms,
            total,
            limit,
            GlossaryPosition::after_due,
        ))
    }

    fn count_glossary<P: rusqlite::Params>(&self, from_where: &str, arguments: P) -> Result<usize> {
        let total: i64 = self
            .connection
            .query_row(
                &format!("SELECT COUNT(*) FROM {from_where}"),
                arguments,
                |row| row.get(0),
            )
            .context("failed to count glossary terms")?;
        Ok(total as usize)
    }
}

pub struct GlossaryPage {
    pub terms: Vec<NodeRecord>,
    pub total: usize,
    pub has_more: bool,
    pub next_position: Option<String>,
}

impl GlossaryPage {
    fn paged(
        mut rows: Vec<NodeRecord>,
        total: usize,
        limit: usize,
        position: impl Fn(&NodeRecord) -> GlossaryPosition,
    ) -> Self {
        let has_more = rows.len() > limit;
        rows.truncate(limit);
        let next_position = if has_more {
            rows.last().map(|record| position(record).token())
        } else {
            None
        };
        Self {
            terms: rows,
            total,
            has_more,
            next_position,
        }
    }

    fn unpaged(rows: Vec<NodeRecord>, total: usize) -> Self {
        Self {
            has_more: total > rows.len(),
            terms: rows,
            total,
            next_position: None,
        }
    }
}

/// Opaque, listing-specific glossary cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryPosition {
    tag: &'static str,
    leading: Option<String>,
    file_path: String,
    line: u32,
}

const TERM_POSITION_TAG: &str = "term";
const DUE_POSITION_TAG: &str = "due";
const POSITION_SEPARATOR: char = '.';

impl GlossaryPosition {
    #[must_use]
    pub fn parse_term(token: &str) -> Option<Self> {
        let position = Self::parse(token, TERM_POSITION_TAG)?;
        position.leading.is_some().then_some(position)
    }

    #[must_use]
    pub fn parse_due(token: &str) -> Option<Self> {
        Self::parse(token, DUE_POSITION_TAG)
    }

    fn after_term(record: &NodeRecord) -> Self {
        Self {
            tag: TERM_POSITION_TAG,
            leading: Some(record.title.clone()),
            file_path: record.file_path.clone(),
            line: record.line,
        }
    }

    fn after_due(record: &NodeRecord) -> Self {
        Self {
            tag: DUE_POSITION_TAG,
            leading: record.sr_due.clone(),
            file_path: record.file_path.clone(),
            line: record.line,
        }
    }

    fn token(&self) -> String {
        // Per-field hex encoding makes the separator unambiguous.
        let mut token = format!(
            "{}{POSITION_SEPARATOR}{}{POSITION_SEPARATOR}{}",
            encode_field(self.tag),
            encode_field(&self.line.to_string()),
            encode_field(&self.file_path)
        );
        if let Some(leading) = &self.leading {
            token.push(POSITION_SEPARATOR);
            token.push_str(&encode_field(leading));
        }
        token
    }

    fn parse(token: &str, tag: &'static str) -> Option<Self> {
        let mut fields = token.split(POSITION_SEPARATOR);
        if decode_field(fields.next()?)? != tag {
            return None;
        }
        let line = decode_field(fields.next()?)?.parse().ok()?;
        let file_path = decode_field(fields.next()?)?;
        let leading = match fields.next() {
            Some(field) => Some(decode_field(field)?),
            None => None,
        };
        if fields.next().is_some() {
            return None;
        }
        Some(Self {
            tag,
            leading,
            file_path,
            line,
        })
    }
}

fn encode_field(field: &str) -> String {
    field.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_field(field: &str) -> Option<String> {
    let digits = field.as_bytes();
    if digits.len() % 2 != 0 {
        return None;
    }
    let bytes = digits
        .chunks(2)
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            Some((high * 16 + low) as u8)
        })
        .collect::<Option<Vec<u8>>>()?;
    String::from_utf8(bytes).ok()
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

#[derive(Debug, Clone, Copy)]
enum FilingSide {
    Earlier,
    Later,
}

fn notes_filed_before_sql() -> String {
    format!(
        "SELECT COUNT(*)
           FROM nodes AS n
          WHERE (n.file_path, n.line) < (?1, ?2)
            AND {}",
        note_where("n"),
    )
}

fn filing_neighbor_sql(side: FilingSide) -> String {
    let (bound, direction) = match side {
        FilingSide::Earlier => ("<", "DESC"),
        FilingSide::Later => (">", "ASC"),
    };
    format!(
        "SELECT n.node_key, n.title
           FROM nodes AS n
          WHERE (n.file_path, n.line) {bound} (?1, ?2)
            AND {}
          ORDER BY n.file_path {direction}, n.line {direction}
          LIMIT 1",
        note_where("n"),
    )
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

/// The metadata search statement. Parameters: 1 the stemmed match, 2 the limit,
/// 3 the whole-term naming probe when `literal` is set.
fn search_nodes_fts_sql(
    sort: Option<&SearchNodesSort>,
    filter: Option<&str>,
    literal: bool,
) -> String {
    let filter = match filter {
        Some(filter) => format!("AND {filter}"),
        None => String::new(),
    };
    let (literal_with, literal_join, literal_rank) = match literal {
        true => (
            probe_with(&[probe_cte("naming", 3)]),
            probe_join("naming", "node_fts.rowid"),
            "naming.rowid IS NULL, ",
        ),
        false => (String::new(), String::new(), ""),
    };
    format!(
        "{literal_with}SELECT {}
           FROM node_fts
           JOIN nodes AS n ON n.id = node_fts.rowid
           {literal_join}
          WHERE node_fts MATCH ?1
            {filter}
          ORDER BY {literal_rank}{}
          LIMIT ?2",
        anchor_select_columns("n"),
        search_nodes_order_by(sort, true),
    )
}

fn search_glossary_sql(filter: &str) -> String {
    format!(
        "{}SELECT {}
           FROM node_fts
           JOIN nodes AS n ON n.id = node_fts.rowid
           {}
          WHERE node_fts MATCH ?1
            AND {filter}
          ORDER BY naming.rowid IS NULL,
                   bm25(node_fts, 1.0, 0.3, 0.2, 0.7, 0.8, 0.4),
                   n.file_path,
                   n.line
          LIMIT ?2",
        probe_with(&[probe_cte("naming", 3)]),
        anchor_select_columns("n"),
        probe_join("naming", "node_fts.rowid"),
    )
}

/// Rank exact headwords, phrases, whole-term naming, whole-term content, BM25,
/// then filing position. Ranking probes do not filter stemmed recall.
fn search_node_content_sql(phrase: bool) -> String {
    let mut probes = vec![probe_cte("naming", 5), probe_cte("anywhere", 6)];
    let (phrase_join, phrase_rank) = match phrase {
        true => {
            probes.push(probe_cte("phrase", 7));
            (
                probe_join("phrase", "node_content_fts.rowid"),
                "phrase.rowid IS NULL, ",
            )
        }
        false => (String::new(), ""),
    };
    // UNION preserves indexed title/alias lookups; NOCASE folds ASCII only.
    // Snippets use this node's body column (2) and control-character delimiters.
    format!(
        "{}SELECT {},
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
           {}
           {phrase_join}
          WHERE node_content_fts MATCH ?1
            AND {}
          ORDER BY headword.id IS NULL,
                   {phrase_rank}naming.rowid IS NULL,
                   anywhere.rowid IS NULL,
                   bm25(node_content_fts),
                   n.file_path,
                   n.line
          LIMIT ?3",
        probe_with(&probes),
        anchor_select_columns("n"),
        probe_join("naming", "node_content_fts.rowid"),
        probe_join("anywhere", "node_content_fts.rowid"),
        note_where("n"),
    )
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

/// Map each anchor to its nearest enclosing canonical note.
pub fn note_owners_by_anchor_key(anchors: &[AnchorRecord]) -> HashMap<String, NodeRecord> {
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

/// Quote FTS5 terms and escape embedded quotes before adding optional prefixes.
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

const PHRASE_NAMING_COLUMNS: &str = "{title aliases}";

/// Unstemmed, unordered conjunctions of whole query terms.
struct LiteralProbes {
    naming: String,
    anywhere: String,
}

/// `Some` exactly when `build_fts_query` is, so probing changes what a search
/// orders and never what it recalls.
fn build_fts_literal_probes(query: &str) -> Option<LiteralProbes> {
    let terms = fts_terms(query);
    if terms.is_empty() {
        return None;
    }
    let anywhere = join_fts_terms(&terms, " AND ", false);
    Some(LiteralProbes {
        naming: format!("{PHRASE_NAMING_COLUMNS} : ({anywhere})"),
        anywhere,
    })
}

fn relevance_literal_probe(query: &str, sort: Option<&SearchNodesSort>) -> Option<String> {
    match sort {
        None | Some(SearchNodesSort::Relevance) => {
            build_fts_literal_probes(query).map(|probes| probes.naming)
        }
        Some(_) => None,
    }
}

/// Materialize probe rowids once per statement, not once per candidate.
fn probe_cte(alias: &str, parameter: usize) -> String {
    format!(
        "{alias} AS MATERIALIZED (SELECT rowid
                                    FROM node_phrase_fts
                                   WHERE node_phrase_fts MATCH ?{parameter})"
    )
}

fn probe_with(probes: &[String]) -> String {
    match probes.is_empty() {
        true => String::new(),
        false => format!("WITH {}\n", probes.join(",\n     ")),
    }
}

/// Ranking evidence is joined, never filtered on: a probe reorders the candidates
/// the stemmed match admitted without removing any of them.
fn probe_join(alias: &str, rowid: &str) -> String {
    format!("LEFT JOIN {alias} ON {alias}.rowid = {rowid}")
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
    use slipbox_index::{DiscoveryPolicy, scan_path_with_policy, scan_root_with_policy};

    use super::{DUE_POSITION_TAG, GlossaryPage, GlossaryPosition, TERM_POSITION_TAG};
    use crate::Database;
    use crate::test_support::indexed_database;

    fn titles(terms: &[slipbox_core::NodeRecord]) -> Vec<String> {
        terms.iter().map(|term| term.title.clone()).collect()
    }

    fn page_titles(page: &GlossaryPage) -> Vec<String> {
        titles(&page.terms)
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

        let page = database.list_glossary_terms(50, None)?;
        assert_eq!(page_titles(&page), vec!["Derivative", "Integral"]);
        assert_eq!(page.total, 2);
        assert!(!page.has_more);
        assert_eq!(page.next_position, None);
        Ok(())
    }

    #[test]
    fn list_respects_limit() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("a.org", &term("Alpha", "", "First.")),
            ("b.org", &term("Beta", "", "Second.")),
            ("c.org", &term("Gamma", "", "Third.")),
        ])?;

        let page = database.list_glossary_terms(2, None)?;
        assert_eq!(page_titles(&page), vec!["Alpha", "Beta"]);
        assert_eq!(page.total, 3);
        assert!(page.has_more);
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

        let page = database.search_glossary("Riemann", 20)?;
        assert_eq!(page_titles(&page), vec!["Riemann integral"]);
        assert_eq!(page.total, 1);
        assert!(!page.has_more);
        Ok(())
    }

    #[test]
    fn search_stems_and_folds_like_note_search() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("d.org", &term("Derivative", "", "Rate of change.")),
            ("g.org", &term("Gödel", "", "Incompleteness.")),
        ])?;

        assert_eq!(
            page_titles(&database.search_glossary("derivatives", 20)?),
            vec!["Derivative"]
        );
        assert_eq!(
            page_titles(&database.search_glossary("Godel", 20)?),
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
        let page = database.search_glossary("!!", 20)?;
        assert_eq!(page_titles(&page), vec!["Alpha", "Beta"]);
        assert_eq!(page.total, 2);
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

        let due = database.glossary_due_terms("2026-07-21", None, 50, None)?;
        assert_eq!(due.total, 3);
        let mut due_titles = page_titles(&due);
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

        let due = database.glossary_due_terms("2026-07-21", None, 2, None)?;
        assert_eq!(page_titles(&due), vec!["A", "B"]);
        assert_eq!(due.total, 3);
        assert!(due.has_more);
        Ok(())
    }

    #[test]
    fn due_search_filters_before_counting_and_paging() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "a.org",
                &term("Alpha", "", "Needle appears only in the definition."),
            ),
            (
                "b.org",
                &term("Beta", ":ROAM_ALIASES: Needle\n", "Alias match."),
            ),
            ("z.org", &term("Needle headword", "", "Headword match.")),
            (
                "future.org",
                &term(
                    "Needle later",
                    ":SR_DUE:  2026-08-01\n:SR_REPS: 1\n",
                    "Not due yet.",
                ),
            ),
        ])?;

        let first = database.glossary_due_terms("2026-07-21", Some("needle"), 1, None)?;
        assert_eq!(page_titles(&first), vec!["Beta"]);
        assert_eq!(first.total, 2);
        assert!(first.has_more);

        let position = read_position(
            first
                .next_position
                .as_deref()
                .expect("the match set continues"),
            GlossaryPosition::parse_due,
        );
        let second =
            database.glossary_due_terms("2026-07-21", Some("needle"), 1, Some(&position))?;
        assert_eq!(page_titles(&second), vec!["Needle headword"]);
        assert_eq!(second.total, 2);
        assert!(!second.has_more);
        Ok(())
    }

    #[test]
    fn search_states_the_cut_rather_than_handing_out_a_position() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("i.org", &term("Riemann integral", "", "Limit of sums.")),
            ("s.org", &term("Riemann sum", "", "A finite sum.")),
        ])?;

        let page = database.search_glossary("Riemann", 1)?;
        assert_eq!(page.terms.len(), 1);
        assert_eq!(page.total, 2);
        assert!(page.has_more);
        assert_eq!(page.next_position, None);
        Ok(())
    }

    #[test]
    fn paging_the_dictionary_listing_reads_every_term_exactly_once() -> Result<()> {
        let sources = (0..7)
            .map(|index| {
                (
                    format!("t{index}.org"),
                    term(&format!("Term {index}"), "", "A term."),
                )
            })
            .collect::<Vec<_>>();
        let files = sources
            .iter()
            .map(|(name, contents)| (name.as_str(), contents.as_str()))
            .collect::<Vec<_>>();
        let (_workspace, database, _root) = indexed_database(&files)?;

        let whole = titles(&database.list_glossary_terms(50, None)?.terms);
        assert_eq!(whole.len(), 7);
        for limit in 1..=4 {
            assert_eq!(paged_titles(&database, limit)?, whole, "limit {limit}");
        }
        Ok(())
    }

    #[test]
    fn paging_holds_together_across_titles_that_differ_only_by_case() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("lower.org", &term("entropy", "", "Lowercase headword.")),
            ("mixed.org", &term("Entropy", "", "Mixed headword.")),
            ("upper.org", &term("ENTROPY", "", "Uppercase headword.")),
            ("z.org", &term("Zeta", "", "Last by title.")),
        ])?;

        // This fixture distinguishes NOCASE order from binary order.
        let whole = titles(&database.list_glossary_terms(50, None)?.terms);
        let mut binary_order = whole.clone();
        binary_order.sort();
        assert_eq!(binary_order, vec!["ENTROPY", "Entropy", "Zeta", "entropy"]);
        assert_ne!(whole, binary_order);
        for limit in 1..=3 {
            assert_eq!(paged_titles(&database, limit)?, whole, "limit {limit}");
        }
        Ok(())
    }

    #[test]
    fn the_due_listing_pages_on_its_own_key_and_reads_unreviewed_terms_once() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("new-a.org", &term("New A", "", "Never reviewed.")),
            ("new-b.org", &term("New B", "", "Never reviewed.")),
            ("new-c.org", &term("New C", "", "Never reviewed.")),
            (
                "dated-a.org",
                &term(
                    "Dated A",
                    ":SR_DUE:  2026-07-10\n:SR_REPS: 1\n",
                    "Earliest.",
                ),
            ),
            (
                "dated-b.org",
                &term("Dated B", ":SR_DUE:  2026-07-15\n:SR_REPS: 2\n", "Later."),
            ),
            (
                "dated-c.org",
                &term(
                    "Dated C",
                    ":SR_DUE:  2026-07-15\n:SR_REPS: 2\n",
                    "Same day.",
                ),
            ),
        ])?;

        let whole = titles(
            &database
                .glossary_due_terms("2026-07-21", None, 50, None)?
                .terms,
        );
        assert_eq!(
            whole,
            vec!["New A", "New B", "New C", "Dated A", "Dated B", "Dated C"]
        );
        for limit in 1..=5 {
            assert_eq!(
                paged_due_titles(&database, "2026-07-21", limit)?,
                whole,
                "limit {limit}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_position_from_one_listing_does_not_resume_the_other() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            ("a.org", &term("Alpha", "", "First.")),
            ("b.org", &term("Beta", "", "Second.")),
        ])?;

        let term_token = database
            .list_glossary_terms(1, None)?
            .next_position
            .expect("a page short of the listing continues");
        assert!(GlossaryPosition::parse_term(&term_token).is_some());
        assert!(GlossaryPosition::parse_due(&term_token).is_none());

        let due_token = database
            .glossary_due_terms("2026-07-21", None, 1, None)?
            .next_position
            .expect("a page short of the listing continues");
        assert!(GlossaryPosition::parse_due(&due_token).is_some());
        assert!(GlossaryPosition::parse_term(&due_token).is_none());
        Ok(())
    }

    #[test]
    fn a_position_round_trips_a_field_holding_the_separator() {
        let term = GlossaryPosition {
            tag: TERM_POSITION_TAG,
            leading: Some("Alpha\u{1f}Beta".to_owned()),
            file_path: "notes/a\u{1f}b.org".to_owned(),
            line: 12,
        };
        assert_eq!(GlossaryPosition::parse_term(&term.token()), Some(term));

        let due = GlossaryPosition {
            tag: DUE_POSITION_TAG,
            leading: None,
            file_path: "notes/a\u{1f}b.org".to_owned(),
            line: 1,
        };
        assert_eq!(GlossaryPosition::parse_due(&due.token()), Some(due));
    }

    #[test]
    fn a_token_no_listing_minted_is_not_a_position() {
        for token in ["", "z", "zz", "abc", "6e6f7065"] {
            assert!(GlossaryPosition::parse_term(token).is_none(), "{token}");
            assert!(GlossaryPosition::parse_due(token).is_none(), "{token}");
        }
    }

    fn paged_titles(database: &Database, limit: usize) -> Result<Vec<String>> {
        let mut page = database.list_glossary_terms(limit, None)?;
        let mut seen = titles(&page.terms);
        while let Some(token) = page.next_position.clone() {
            let position = read_position(&token, GlossaryPosition::parse_term);
            page = database.list_glossary_terms(limit, Some(&position))?;
            assert!(!page.terms.is_empty(), "a continued page holds a term");
            seen.extend(titles(&page.terms));
        }
        Ok(seen)
    }

    fn paged_due_titles(database: &Database, today: &str, limit: usize) -> Result<Vec<String>> {
        let mut page = database.glossary_due_terms(today, None, limit, None)?;
        let mut seen = titles(&page.terms);
        while let Some(token) = page.next_position.clone() {
            let position = read_position(&token, GlossaryPosition::parse_due);
            page = database.glossary_due_terms(today, None, limit, Some(&position))?;
            assert!(!page.terms.is_empty(), "a continued page holds a term");
            seen.extend(titles(&page.terms));
        }
        Ok(seen)
    }

    fn read_position(
        token: &str,
        parse: impl Fn(&str) -> Option<GlossaryPosition>,
    ) -> GlossaryPosition {
        parse(token).expect("a listing reads back the position it minted")
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
    fn content_search_includes_glossary_titles() -> Result<()> {
        let glossary = term(
            "Convex duality",
            "",
            "A relation between optimization problems.",
        );
        let (_workspace, database, _root) =
            indexed_database(&[("duality.org", glossary.as_str())])?;

        let hits = database.search_node_content("Convex duality", 20)?;
        assert_eq!(content_titles(&hits), vec!["Convex duality"]);
        assert!(hits[0].node.glossary);
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
            .list_glossary_terms(10, None)?
            .terms
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
            database.list_glossary_terms(10, None)?.terms.is_empty(),
            "a forced rebuild empties the derived index"
        );

        // Re-sync from Org (the source of truth) repopulates every column.
        let files = scan_root_with_policy(&root, &DiscoveryPolicy::default())?;
        database.sync_index(&files)?;

        let after = database
            .list_glossary_terms(10, None)?
            .terms
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

    fn filing_fixture() -> [(&'static str, &'static str); 3] {
        [
            ("a.org", "#+title: Alpha\n\nFirst.\n"),
            (
                "b.org",
                "#+title: Beta\n\nSecond.\n\n* Ordinary heading\n\nNo identifier.\n\n* Filed child\n:PROPERTIES:\n:ID: filed-child\n:END:\n\nA note inside Beta.\n",
            ),
            ("c.org", "#+title: Gamma\n\nThird.\n"),
        ]
    }

    fn filed_notes(database: &crate::Database) -> Result<Vec<slipbox_core::NodeRecord>> {
        database.search_nodes("", 200, None)
    }

    fn place(
        database: &crate::Database,
        note: &slipbox_core::NodeRecord,
    ) -> Result<slipbox_core::NotePlaceResult> {
        Ok(database
            .note_place(&note.node_key)?
            .expect("an indexed note has a place"))
    }

    fn query_plan(database: &crate::Database, sql: &str) -> Result<String> {
        let mut statement = database
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let rows = statement.query_map(rusqlite::params!["", 0], |row| row.get::<_, String>(3))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?.join("\n"))
    }

    /// An EXPLAIN row with parent links for checking probe ancestry.
    struct PlanNode {
        id: i64,
        parent: i64,
        detail: String,
    }

    fn search_plan(
        database: &crate::Database,
        sql: &str,
        arguments: &[&str],
    ) -> Result<Vec<PlanNode>> {
        let mut statement = database
            .connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
        let rows = statement.query_map(rusqlite::params_from_iter(arguments), |row| {
            Ok(PlanNode {
                id: row.get(0)?,
                parent: row.get(1)?,
                detail: row.get(3)?,
            })
        })?;
        let plan = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        println!("--- {sql}\n-- arguments: {arguments:?}");
        for node in &plan {
            println!("{:>3} <- {:>3}  {}", node.id, node.parent, node.detail);
        }
        println!();
        Ok(plan)
    }

    fn empty_arguments(parameters: usize) -> Vec<&'static str> {
        vec![""; parameters]
    }

    fn details(plan: &[PlanNode]) -> Vec<&str> {
        plan.iter().map(|node| node.detail.as_str()).collect()
    }

    /// FTS5 reports MATCH and rowid equality constraints as `M` and `=`.
    fn fts_constraints(detail: &str) -> Option<&str> {
        detail
            .split_once("VIRTUAL TABLE INDEX 0:")
            .and_then(|(_, constraints)| constraints.split_whitespace().next())
    }

    fn admitted_by_a_match(detail: &str) -> bool {
        fts_constraints(detail).is_some_and(|constraints| constraints.contains('M'))
    }

    fn driven_by_an_outer_rowid(detail: &str) -> bool {
        fts_constraints(detail).is_some_and(|constraints| constraints.contains('='))
    }

    fn full_text_reads(plan: &[PlanNode]) -> Vec<&PlanNode> {
        plan.iter()
            .filter(|node| node.detail.contains("_fts"))
            .collect()
    }

    fn ancestors<'plan>(plan: &'plan [PlanNode], node: &PlanNode) -> Vec<&'plan PlanNode> {
        let mut chain = Vec::new();
        let mut parent = node.parent;
        while let Some(next) = plan.iter().find(|candidate| candidate.id == parent) {
            chain.push(next);
            parent = next.parent;
        }
        chain
    }

    /// Check MATCH admission without rowid pushdown or correlated FTS ancestry.
    /// Production probes must also pass `probe_materialized_once`.
    fn full_text_reads_run_once(plan: &[PlanNode]) -> bool {
        full_text_reads(plan).into_iter().all(|node| {
            admitted_by_a_match(&node.detail)
                && !driven_by_an_outer_rowid(&node.detail)
                && ancestors(plan, node)
                    .iter()
                    .all(|ancestor| ancestor.detail.starts_with("MATERIALIZE "))
        })
    }

    /// A statement-scope materialization containing one indexed FTS read.
    fn probe_materialized_once(plan: &[PlanNode], alias: &str) -> bool {
        let materialize = format!("MATERIALIZE {alias}");
        plan.iter()
            .filter(|node| node.detail == materialize && node.parent == 0)
            .any(|root| {
                let reads = full_text_reads(plan)
                    .into_iter()
                    .filter(|node| {
                        ancestors(plan, node)
                            .iter()
                            .any(|ancestor| ancestor.id == root.id)
                    })
                    .collect::<Vec<_>>();
                reads.len() == 1
                    && reads.iter().all(|node| {
                        node.detail.contains("node_phrase_fts")
                            && admitted_by_a_match(&node.detail)
                            && !driven_by_an_outer_rowid(&node.detail)
                    })
            })
    }

    #[test]
    fn the_filing_ordinal_counts_from_one_over_every_note_the_index_holds() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;
        let filed = filed_notes(&database)?;
        assert_eq!(
            titles(&filed),
            vec!["Alpha", "Beta", "Filed child", "Gamma"]
        );

        for (index, note) in filed.iter().enumerate() {
            let place = place(&database, note)?;
            assert_eq!(place.ordinal, index as u64 + 1);
            assert_eq!(place.total, database.notes_indexed()?);
        }
        Ok(())
    }

    #[test]
    fn the_first_note_reports_no_earlier_neighbor_and_the_last_no_later_one() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;
        let filed = filed_notes(&database)?;
        let first = place(&database, &filed[0])?;
        let last = place(&database, &filed[filed.len() - 1])?;

        assert_eq!(first.ordinal, 1);
        assert!(first.earlier.is_none());
        assert_eq!(
            first.later.map(|neighbor| neighbor.title),
            Some("Beta".to_owned())
        );
        assert_eq!(last.ordinal, last.total);
        assert!(last.later.is_none());
        assert_eq!(
            last.earlier.map(|neighbor| neighbor.title),
            Some("Filed child".to_owned())
        );
        Ok(())
    }

    #[test]
    fn two_notes_in_one_file_are_filed_by_line() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;
        let filed = filed_notes(&database)?;
        assert_eq!(filed[1].file_path, filed[2].file_path);
        assert!(filed[1].line < filed[2].line);

        let earlier = place(&database, &filed[1])?;
        let later = place(&database, &filed[2])?;
        assert_eq!((earlier.ordinal, later.ordinal), (2, 3));
        assert_eq!(
            earlier.later.map(|neighbor| neighbor.title),
            Some("Filed child".to_owned())
        );
        assert_eq!(
            later.earlier.map(|neighbor| neighbor.title),
            Some("Beta".to_owned())
        );
        assert_eq!(
            later.later.map(|neighbor| neighbor.title),
            Some("Gamma".to_owned())
        );
        Ok(())
    }

    #[test]
    fn a_filing_neighbor_carries_the_key_and_title_that_name_it() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;
        let filed = filed_notes(&database)?;
        let place = place(&database, &filed[1])?;

        let earlier = place
            .earlier
            .expect("a middle note has an earlier neighbor");
        assert_eq!(earlier.node_key, filed[0].node_key);
        assert_eq!(earlier.title, filed[0].title);
        let later = place.later.expect("a middle note has a later neighbor");
        assert_eq!(later.node_key, filed[2].node_key);
        assert_eq!(later.title, filed[2].title);
        Ok(())
    }

    #[test]
    fn a_key_the_index_does_not_hold_has_no_place() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;

        assert!(database.note_place("heading:missing.org:9999")?.is_none());
        Ok(())
    }

    #[test]
    fn an_ordinary_heading_takes_no_place_in_the_filing_order() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;
        let filed = filed_notes(&database)?;
        let ordinary = database
            .anchors_in_file(&filed[1].file_path)?
            .into_iter()
            .find(|anchor| anchor.title == "Ordinary heading")
            .expect("the fixture heading is indexed as an anchor");

        assert!(database.note_place(&ordinary.node_key)?.is_none());
        let place = place(&database, &filed[1])?;
        assert_eq!(place.total, 4);
        assert_eq!(
            place.later.map(|neighbor| neighbor.title),
            Some("Filed child".to_owned())
        );
        Ok(())
    }

    #[test]
    fn the_filing_seeks_walk_the_ordered_index_without_sorting() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&filing_fixture())?;

        for sql in [
            super::notes_filed_before_sql(),
            super::filing_neighbor_sql(super::FilingSide::Earlier),
            super::filing_neighbor_sql(super::FilingSide::Later),
        ] {
            let plan = query_plan(&database, &sql)?;
            assert!(
                plan.contains("SEARCH n USING INDEX idx_nodes_file_path_line_level"),
                "the plan must seek the filing-order index: {plan}"
            );
            assert!(!plan.contains("SCAN"), "the plan must not scan: {plan}");
            assert!(
                !plan.contains("TEMP B-TREE"),
                "the plan must not sort: {plan}"
            );
        }
        Ok(())
    }

    fn literal_fixture() -> [(&'static str, &'static str); 4] {
        [
            (
                "architecture.org",
                "#+title: The transformer architecture and its attention blocks\n\nA sequence model assembled from stacked attention blocks.\n",
            ),
            (
                "linear.org",
                "#+title: Linear transformation\n\nA transformation transforms a vector space, one transformation composes with another transformation, and a transformation of a transformation transforms again.\n",
            ),
            (
                "coordinates.org",
                "#+title: Coordinate changes\n\nA change of coordinates transforms components: transforming twice transforms back, and each transformation of the basis transforms the matrix.\n",
            ),
            (
                "sequences.org",
                "#+title: Sequence models\n\nRecurrent nets came first; a transformer replaced them for long contexts.\n",
            ),
        ]
    }

    #[test]
    fn content_search_prefers_a_whole_term_in_the_title_over_a_repeated_stem() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&literal_fixture())?;

        let ranked = content_titles(&database.search_node_content("transformer", 20)?);
        assert_eq!(
            ranked.first().map(String::as_str),
            Some("The transformer architecture and its attention blocks"),
            "the title naming the whole term outranks every stem-only match: {ranked:?}"
        );
        assert_eq!(
            ranked.get(1).map(String::as_str),
            Some("Sequence models"),
            "prose spelling the whole term outranks a stem-only match: {ranked:?}"
        );
        let mut stemmed = ranked[2..].to_vec();
        stemmed.sort();
        assert_eq!(
            stemmed,
            vec!["Coordinate changes", "Linear transformation"],
            "stem-only matches keep their recall behind the literal ones"
        );
        Ok(())
    }

    #[test]
    fn content_search_keeps_stem_recall_when_nothing_spells_the_term() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&literal_fixture())?;

        let mut every = content_titles(&database.search_node_content("transforming", 20)?);
        every.sort();
        assert_eq!(
            every,
            vec![
                "Coordinate changes",
                "Linear transformation",
                "Sequence models",
                "The transformer architecture and its attention blocks",
            ],
            "a stemmed query still reaches every related spelling"
        );
        Ok(())
    }

    #[test]
    fn a_whole_term_is_not_evidence_for_a_longer_word_containing_it() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "longer.org",
                "#+title: Transformer blocks in practice\n\nA stack of attention blocks.\n",
            ),
            (
                "shorter.org",
                "#+title: Transform of a linear map\n\nA change of basis written out.\n",
            ),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("transform", 20)?)
                .first()
                .map(String::as_str),
            Some("Transform of a linear map"),
            "the query term is whole in one title and merely contained in the other"
        );
        assert_eq!(
            content_titles(&database.search_node_content("transformer", 20)?)
                .first()
                .map(String::as_str),
            Some("Transformer blocks in practice")
        );
        Ok(())
    }

    #[test]
    fn content_search_prefers_a_whole_term_in_an_alias() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "attention.org",
                "#+title: Attention is all you need\n:PROPERTIES:\n:ROAM_ALIASES: \"Transformers and attention\"\n:END:\n\nA sequence model built from attention alone.\n",
            ),
            literal_fixture()[1],
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("transformers", 20)?),
            vec!["Attention is all you need", "Linear transformation"],
            "an alias holding the whole term outranks a repeated stem, and neither note is the query's exact headword"
        );
        Ok(())
    }

    #[test]
    fn content_search_mixes_a_whole_term_with_a_stemmed_one() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            literal_fixture()[0],
            (
                "attn.org",
                "#+title: Attention and transformations\n\nAttention weights a transformation, and each transformation follows the transformations before it.\n",
            ),
        ])?;

        assert_eq!(
            content_titles(&database.search_node_content("transformer attention", 20)?),
            vec![
                "The transformer architecture and its attention blocks",
                "Attention and transformations",
            ],
            "a query mixing a whole term with a stemmed one keeps both matches and prefers the literal title"
        );
        Ok(())
    }

    const CROWDED_WINNER: &str = "The transformer architecture in practice";

    fn crowded_fixture() -> Vec<(String, String)> {
        let mut fixture = (0..40)
            .map(|index| {
                (
                    format!("distractor-{index:02}.org"),
                    format!(
                        "#+title: Transformation notes {index:02}\n\nA transformation transforms a transformation, transforming a transformation into another transformation.\n"
                    ),
                )
            })
            .collect::<Vec<_>>();
        fixture.push((
            "zz-winner.org".to_owned(),
            format!(
                "#+title: {CROWDED_WINNER}\n\nA long discussion of stacked attention blocks that never repeats the word in its prose, wandering instead through residual streams, layer normalization, positional encodings, and the many other details a working implementation settles before it runs.\n"
            ),
        ));
        fixture
    }

    fn crowded_database() -> Result<(tempfile::TempDir, crate::Database, std::path::PathBuf)> {
        let fixture = crowded_fixture();
        let files = fixture
            .iter()
            .map(|(name, body)| (name.as_str(), body.as_str()))
            .collect::<Vec<_>>();
        indexed_database(&files)
    }

    #[test]
    fn content_search_reaches_a_literal_winner_the_relevance_order_buried() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;

        assert_eq!(
            content_titles(&database.search_node_content("transformer", 1)?),
            vec![CROWDED_WINNER],
            "the limit applies after the canonical ordering, so the one hit is the literal winner"
        );
        Ok(())
    }

    #[test]
    fn one_statement_serves_a_broad_query_and_a_selective_one() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;

        let broad = database.search_node_content("transformation", 5)?;
        assert_eq!(broad.len(), 5, "a broad query stays bounded by its limit");
        assert!(
            content_titles(&broad)
                .iter()
                .all(|title| title.starts_with("Transformation notes")),
            "the notes spelling the broad term out hold the page: {:?}",
            content_titles(&broad)
        );

        assert_eq!(
            content_titles(&database.search_node_content("residual", 5)?),
            vec![CROWDED_WINNER],
            "a selective term returns only the note carrying it"
        );
        assert_eq!(
            content_titles(&database.search_node_content("transformer", 5)?)
                .first()
                .map(String::as_str),
            Some(CROWDED_WINNER),
            "the literal winner leads the page a common stem would have filled"
        );
        Ok(())
    }

    #[test]
    fn every_search_statement_materializes_its_probes_at_statement_scope() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;
        let note_where = super::note_where("n");

        for (sql, parameters, probes) in [
            (
                super::search_node_content_sql(true),
                7,
                vec!["naming", "anywhere", "phrase"],
            ),
            (
                super::search_node_content_sql(false),
                6,
                vec!["naming", "anywhere"],
            ),
            (
                super::search_nodes_fts_sql(None, Some(&note_where), true),
                3,
                vec!["naming"],
            ),
            (
                super::search_glossary_sql(&super::glossary_where("n")),
                3,
                vec!["naming"],
            ),
        ] {
            let plan = search_plan(&database, &sql, &empty_arguments(parameters))?;
            for alias in &probes {
                assert!(
                    probe_materialized_once(&plan, alias),
                    "probe {alias} must be materialized once at statement scope: {:?}",
                    details(&plan)
                );
                assert!(
                    details(&plan).iter().any(|detail| {
                        detail.starts_with(&format!("SEARCH {alias} "))
                            && detail.contains("(rowid=?)")
                    }),
                    "probe {alias} must be read back by rowid: {:?}",
                    details(&plan)
                );
            }
            assert_eq!(
                full_text_reads(&plan)
                    .iter()
                    .filter(|node| node.detail.contains("node_phrase_fts"))
                    .count(),
                probes.len(),
                "the statement reads the probe table once per probe: {:?}",
                details(&plan)
            );
            assert!(
                full_text_reads_run_once(&plan),
                "every full-text read must be an indexed MATCH evaluated once: {:?}",
                details(&plan)
            );
            assert!(
                details(&plan)
                    .iter()
                    .any(|detail| detail.contains("SEARCH n USING INTEGER PRIMARY KEY")),
                "the matched rowid must resolve its node by primary key: {:?}",
                details(&plan)
            );
            assert!(
                !details(&plan)
                    .iter()
                    .any(|detail| detail.trim() == "SCAN n"),
                "no statement may scan the notes table: {:?}",
                details(&plan)
            );
            // LIMIT bounds the page, not the sort over admitted candidates.
            assert!(
                details(&plan)
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
                "candidate ordering is a sort over every admitted row: {:?}",
                details(&plan)
            );
        }

        let content_plan = search_plan(
            &database,
            &super::search_node_content_sql(true),
            &empty_arguments(7),
        )?;
        assert!(
            details(&content_plan)
                .iter()
                .any(|detail| detail.contains("idx_nodes_title_nocase"))
                && details(&content_plan)
                    .iter()
                    .any(|detail| detail.contains("idx_aliases_alias_nocase")),
            "the headword equality tests must reach their folded indexes: {:?}",
            details(&content_plan)
        );

        let sorted_plan = search_plan(
            &database,
            &super::search_nodes_fts_sql(
                Some(&slipbox_core::SearchNodesSort::Title),
                Some(&note_where),
                false,
            ),
            &empty_arguments(2),
        )?;
        assert!(
            !details(&sorted_plan)
                .iter()
                .any(|detail| detail.contains("node_phrase_fts")),
            "an explicit sort carries no probe at all: {:?}",
            details(&sorted_plan)
        );
        Ok(())
    }

    #[test]
    fn a_common_query_and_a_selective_one_are_admitted_the_same_way() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;
        let sql = super::search_node_content_sql(false);
        let budget = super::SNIPPET_TOKEN_BUDGET.to_string();

        let mut plans = Vec::new();
        for query in ["transformation", "residual"] {
            let matched = super::build_fts_query(query).expect("a term to match");
            let probes = super::build_fts_literal_probes(query).expect("a term to probe");
            let plan = search_plan(
                &database,
                &sql,
                &[
                    &matched,
                    &budget,
                    "5",
                    query,
                    &probes.naming,
                    &probes.anywhere,
                ],
            )?;
            for alias in ["naming", "anywhere"] {
                assert!(
                    probe_materialized_once(&plan, alias),
                    "probe {alias} must be materialized once for {query}: {:?}",
                    details(&plan)
                );
            }
            assert!(
                full_text_reads_run_once(&plan),
                "every full-text read must be an indexed MATCH evaluated once: {:?}",
                details(&plan)
            );
            plans.push(details(&plan).join("\n"));
        }

        assert_eq!(
            plans[0], plans[1],
            "selectivity decides how many rows the MATCH admits, not how they are read"
        );
        Ok(())
    }

    #[test]
    fn a_scanning_statement_reads_differently_from_the_search_statements() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;

        let unconstrained = search_plan(&database, "SELECT rowid FROM node_phrase_fts", &[])?;
        assert!(
            !full_text_reads(&unconstrained).is_empty(),
            "the control must reach the probe table: {:?}",
            details(&unconstrained)
        );
        assert!(
            !details(&unconstrained)
                .iter()
                .any(|detail| admitted_by_a_match(detail)),
            "the control must show an unconstrained lookup: {:?}",
            details(&unconstrained)
        );
        assert!(
            !full_text_reads_run_once(&unconstrained),
            "the one-shot check must reject an unconstrained lookup: {:?}",
            details(&unconstrained)
        );

        let scanning = search_plan(
            &database,
            &format!(
                "SELECT n.node_key
                   FROM nodes AS n
                  WHERE n.title LIKE ?1
                    AND {}",
                super::note_where("n")
            ),
            &["%transformer%"],
        )?;
        assert!(
            details(&scanning)
                .iter()
                .any(|detail| detail.trim() == "SCAN n"),
            "the control must scan the notes table: {:?}",
            details(&scanning)
        );
        assert!(
            !details(&scanning)
                .iter()
                .any(|detail| detail.contains("USING INTEGER PRIMARY KEY")),
            "the control must not resolve by primary key: {:?}",
            details(&scanning)
        );
        Ok(())
    }

    #[test]
    fn a_repeated_full_text_lookup_reads_differently_from_a_materialized_probe() -> Result<()> {
        let (_workspace, database, _root) = crowded_database()?;
        let matched = super::build_fts_query("transformation").expect("a term to match");
        let probes = super::build_fts_literal_probes("transformation").expect("a term to probe");

        // `rowid + 0` keeps the correlation while denying FTS5 a rowid constraint, so
        // the lookup repeats for every outer candidate with a MATCH and no `=`.
        let correlated = search_plan(
            &database,
            "SELECT n.node_key
               FROM node_content_fts
               JOIN nodes AS n ON n.id = node_content_fts.rowid
              WHERE node_content_fts MATCH ?1
                AND EXISTS (SELECT 1
                              FROM node_phrase_fts
                             WHERE node_phrase_fts MATCH ?2
                               AND node_phrase_fts.rowid + 0 = node_content_fts.rowid)",
            &[&matched, &probes.naming],
        )?;
        let repeated = full_text_reads(&correlated)
            .into_iter()
            .find(|node| node.detail.contains("node_phrase_fts"))
            .expect("the control reads the probe table");
        assert!(
            admitted_by_a_match(&repeated.detail) && !driven_by_an_outer_rowid(&repeated.detail),
            "the control must keep a MATCH and no rowid constraint: {}",
            repeated.detail
        );
        assert!(
            ancestors(&correlated, repeated)
                .iter()
                .any(|ancestor| ancestor.detail.contains("CORRELATED")),
            "the control must be driven by the outer query: {:?}",
            details(&correlated)
        );
        assert!(
            !full_text_reads_run_once(&correlated),
            "the one-shot check must reject a correlated MATCH: {:?}",
            details(&correlated)
        );

        // A flattened LEFT JOIN permits candidate-driven rowid lookups.
        let inline = search_plan(
            &database,
            "SELECT n.node_key
               FROM node_content_fts
               JOIN nodes AS n ON n.id = node_content_fts.rowid
               LEFT JOIN (SELECT rowid
                            FROM node_phrase_fts
                           WHERE node_phrase_fts MATCH ?2) AS naming
                      ON naming.rowid = node_content_fts.rowid
              WHERE node_content_fts MATCH ?1",
            &[&matched, &probes.naming],
        )?;
        assert!(
            full_text_reads(&inline)
                .iter()
                .any(|node| driven_by_an_outer_rowid(&node.detail)),
            "the inline shape must show a pushed-down rowid: {:?}",
            details(&inline)
        );
        assert!(
            !full_text_reads_run_once(&inline),
            "the one-shot check must reject the inline shape: {:?}",
            details(&inline)
        );
        assert!(
            !probe_materialized_once(&inline, "naming"),
            "the inline shape materializes nothing: {:?}",
            details(&inline)
        );
        Ok(())
    }

    #[test]
    fn content_search_orders_equivalent_evidence_by_filing_position() -> Result<()> {
        let note = |id: &str| {
            format!(
                "#+title: Transformer notes\n\nThe transformer is described here.\n\n* Later section\n:PROPERTIES:\n:ID: {id}\n:END:\n\nThe transformer is described here.\n"
            )
        };
        let (first, second) = (note("first-heading"), note("second-heading"));
        let (_workspace, database, _root) = indexed_database(&[
            ("b-second.org", second.as_str()),
            ("a-first.org", first.as_str()),
        ])?;

        let hits = database.search_node_content("transformer", 20)?;
        assert_eq!(
            content_titles(&hits),
            vec![
                "Transformer notes",
                "Transformer notes",
                "Later section",
                "Later section"
            ],
            "the notes naming the term outrank the ones carrying it in prose alone"
        );
        assert_eq!(
            hits.iter()
                .map(|hit| hit.node.file_path.as_str())
                .collect::<Vec<_>>(),
            vec!["a-first.org", "b-second.org", "a-first.org", "b-second.org"],
            "equivalent evidence orders by filing position"
        );
        Ok(())
    }

    fn metadata_literal_fixture() -> [(&'static str, &'static str); 2] {
        [
            (
                "block.org",
                "#+title: Transformer block in a deep network\n\nA layer of attention and a feed-forward map.\n",
            ),
            (
                "transformations.org",
                "#+title: Transformations\n#+filetags: :transformation:transforms:\n:PROPERTIES:\n:ROAM_ALIASES: \"transformation transforms\"\n:END:\n\nRepeated in the metadata rather than the prose.\n",
            ),
        ]
    }

    #[test]
    fn metadata_search_prefers_a_whole_term_in_the_title() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&metadata_literal_fixture())?;

        assert_eq!(
            titles(&database.search_nodes("transformer", 20, None)?),
            vec!["Transformer block in a deep network", "Transformations"],
            "the title naming the whole term outranks a stem repeated across metadata columns"
        );
        Ok(())
    }

    #[test]
    fn an_explicit_metadata_sort_keeps_its_own_order() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&metadata_literal_fixture())?;

        assert_eq!(
            titles(&database.search_nodes(
                "transformer",
                20,
                Some(slipbox_core::SearchNodesSort::Title)
            )?),
            vec!["Transformations", "Transformer block in a deep network"],
            "an explicit sort states the order and relevance evidence does not touch it"
        );
        assert_eq!(
            titles(&database.search_nodes(
                "transformer",
                20,
                Some(slipbox_core::SearchNodesSort::File)
            )?),
            vec!["Transformer block in a deep network", "Transformations"]
        );
        Ok(())
    }

    #[test]
    fn glossary_search_prefers_a_headword_holding_the_whole_term() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "block.org",
                &term(
                    "Transformer block in a deep network",
                    "",
                    "A layer of attention and a feed-forward map.",
                ),
            ),
            (
                "transformations.org",
                "#+title: Transformations\n#+glossary: t\n#+filetags: :transformation:transforms:\n:PROPERTIES:\n:ROAM_ALIASES: \"transformation transforms\"\n:END:\n\nRepeated in the metadata rather than the prose.\n",
            ),
        ])?;

        let page = database.search_glossary("transformer", 20)?;
        assert_eq!(
            page_titles(&page),
            vec!["Transformer block in a deep network", "Transformations"],
            "the headword naming the whole term outranks one merely sharing its stem"
        );
        assert_eq!(page.total, 2, "the probe orders the page, never filters it");
        Ok(())
    }

    #[test]
    fn whole_term_evidence_survives_a_derived_schema_rebuild() -> Result<()> {
        let (_workspace, mut database, root) =
            indexed_database(&[literal_fixture()[0], literal_fixture()[1]])?;

        assert_eq!(
            literal_first(&database)?.as_deref(),
            Some("The transformer architecture and its attention blocks")
        );

        database
            .connection
            .execute_batch("PRAGMA user_version = 0;")?;
        database.migrate()?;
        assert!(
            database.search_node_content("transformer", 20)?.is_empty(),
            "a forced rebuild empties the probe index with the rest of the derived schema"
        );

        let files = scan_root_with_policy(&root, &DiscoveryPolicy::default())?;
        database.sync_index(&files)?;
        assert_eq!(
            literal_first(&database)?.as_deref(),
            Some("The transformer architecture and its attention blocks"),
            "re-syncing from Org restores the evidence the rebuild dropped"
        );
        Ok(())
    }

    const NAMING_FILE: &str = "#+title: The transformer architecture and its attention blocks\n\nA sequence model assembled from stacked attention blocks.\n\n* Transformer residual streams\n:PROPERTIES:\n:ID: 6a26f38c-0000-4000-8000-000000000001\n:END:\n\nEach block adds to the stream it read.\n";
    const NAMING_FILE_WITHOUT_THE_TERM: &str = "#+title: The attention architecture\n\nA sequence model assembled from stacked attention blocks that it transforms.\n";
    const STEM_FILE: &str = "#+title: Linear transformation\n\nA transformation transforms a vector space, one transformation composes with another transformation, and a transformation of a transformation transforms again.\n";
    const STEM_FILE_WITH_A_NAMING_ALIAS: &str = "#+title: Linear maps\n:PROPERTIES:\n:ROAM_ALIASES: \"Linear transformer maps\"\n:END:\n\nA transformation transforms a vector space, one transformation composes with another transformation.\n";

    fn literal_first(database: &crate::Database) -> Result<Option<String>> {
        let ranked = content_titles(&database.search_node_content("transformer", 20)?);
        Ok(ranked.into_iter().next())
    }

    fn naming_evidence(database: &crate::Database, query: &str) -> Result<i64> {
        let probes = super::build_fts_literal_probes(query).expect("a term to probe");
        let count = database.connection.query_row(
            "SELECT COUNT(*) FROM node_phrase_fts WHERE node_phrase_fts MATCH ?1",
            [&probes.naming],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn orphaned_probe_rows(database: &crate::Database) -> Result<i64> {
        let count = database.connection.query_row(
            "SELECT COUNT(*)
               FROM node_phrase_fts
              WHERE rowid NOT IN (SELECT id FROM nodes)",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn indexed_paths(database: &crate::Database) -> Result<Vec<String>> {
        let mut statement = database
            .connection
            .prepare("SELECT path FROM files ORDER BY path")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        let paths = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(paths)
    }

    fn refresh_one_file(
        database: &mut crate::Database,
        root: &std::path::Path,
        name: &str,
    ) -> Result<()> {
        let file = scan_path_with_policy(root, &root.join(name), &DiscoveryPolicy::default())?;
        database.sync_file_index(&file)?;
        Ok(())
    }

    #[test]
    fn one_file_refreshed_alone_moves_its_whole_term_evidence() -> Result<()> {
        let (_workspace, mut database, root) = indexed_database(&[
            ("architecture.org", NAMING_FILE),
            ("linear.org", STEM_FILE),
            ("coordinates.org", literal_fixture()[2].1),
        ])?;

        assert_eq!(
            naming_evidence(&database, "transformer")?,
            2,
            "the file and its heading both name the term"
        );
        let ranked = content_titles(&database.search_node_content("transformer", 20)?);
        assert!(
            ranked.contains(&"Transformer residual streams".to_owned())
                && ranked
                    .contains(&"The transformer architecture and its attention blocks".to_owned()),
            "both naming nodes answer the search: {ranked:?}"
        );

        std::fs::write(root.join("architecture.org"), NAMING_FILE_WITHOUT_THE_TERM)?;
        refresh_one_file(&mut database, &root, "architecture.org")?;

        assert_eq!(
            naming_evidence(&database, "transformer")?,
            0,
            "the evidence that file supplied is gone with the words that carried it"
        );
        let ranked = content_titles(&database.search_node_content("transformer", 20)?);
        assert!(
            ranked.contains(&"The attention architecture".to_owned()),
            "the refreshed file reads back as it was written: {ranked:?}"
        );
        assert!(
            !ranked
                .iter()
                .any(|title| title == "Transformer residual streams"),
            "the heading the edit deleted does not linger: {ranked:?}"
        );
        assert!(
            ranked.contains(&"Coordinate changes".to_owned()),
            "a file the refresh did not name keeps its rows: {ranked:?}"
        );

        std::fs::write(root.join("ghost.org"), "#+title: Ghost transformer\n")?;
        std::fs::write(root.join("linear.org"), STEM_FILE_WITH_A_NAMING_ALIAS)?;
        refresh_one_file(&mut database, &root, "linear.org")?;

        assert_eq!(naming_evidence(&database, "transformer")?, 1);
        assert_eq!(
            literal_first(&database)?.as_deref(),
            Some("Linear maps"),
            "an alias added to one file leads the ranking as soon as that file is refreshed"
        );
        assert_eq!(
            indexed_paths(&database)?,
            vec!["architecture.org", "coordinates.org", "linear.org"],
            "refreshing one file imports no other, however discoverable"
        );
        assert!(
            content_titles(&database.search_node_content("Ghost", 20)?).is_empty(),
            "a file that was never indexed stays out of the results"
        );
        Ok(())
    }

    #[test]
    fn removing_one_file_drops_its_whole_term_evidence() -> Result<()> {
        let (_workspace, mut database, _root) = indexed_database(&[
            ("architecture.org", NAMING_FILE),
            ("coordinates.org", literal_fixture()[2].1),
        ])?;
        assert_eq!(naming_evidence(&database, "transformer")?, 2);

        database.remove_file_index("architecture.org")?;

        assert_eq!(
            naming_evidence(&database, "transformer")?,
            0,
            "removing the file removes the rowids its probes matched"
        );
        assert_eq!(
            orphaned_probe_rows(&database)?,
            0,
            "no probe row outlives the node it described"
        );
        assert_eq!(
            content_titles(&database.search_node_content("transformer", 20)?),
            vec!["Coordinate changes"],
            "the file left indexed still answers the search"
        );
        assert_eq!(indexed_paths(&database)?, vec!["coordinates.org"]);
        Ok(())
    }
}

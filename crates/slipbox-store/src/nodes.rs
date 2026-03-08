use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;

use slipbox_core::{NodeKind, NodeRecord};

use crate::Database;

impl Database {
    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<NodeRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if let Some(fts_query) = build_fts_query(query) {
            let mut statement = self.connection.prepare(
                "SELECT n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                  ORDER BY bm25(node_fts, 1.0, 0.3, 0.2, 0.7, 0.8, 0.4), n.file_path, n.line
                  LIMIT ?2",
            )?;
            let rows = statement.query_map(params![fts_query, limit], row_to_node)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read search results")
        } else {
            let mut statement = self.connection.prepare(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        aliases_json,
                        tags_json,
                        refs_json,
                        todo_keyword,
                        scheduled_for,
                        deadline_for,
                        closed_at,
                        level,
                        line,
                        kind
                   FROM nodes
                  ORDER BY file_path, line
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_node)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read node listing")
        }
    }

    pub fn random_node(&self) -> Result<Option<NodeRecord>> {
        let Some((min_id, max_id)) = self
            .connection
            .query_row(
                "SELECT MIN(id), MAX(id)
                   FROM nodes",
                [],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .context("failed to determine node ID bounds")?
        else {
            return Ok(None);
        };

        let (Some(min_id), Some(max_id)) = (min_id, max_id) else {
            return Ok(None);
        };

        self.connection
            .query_row(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        aliases_json,
                        tags_json,
                        refs_json,
                        todo_keyword,
                        scheduled_for,
                        deadline_for,
                        closed_at,
                        level,
                        line,
                        kind
                   FROM nodes
                  WHERE id >= ((ABS(random()) % (?2 - ?1 + 1)) + ?1)
                  ORDER BY id
                  LIMIT 1",
                params![min_id, max_id],
                row_to_node,
            )
            .optional()
            .context("failed to fetch random node")
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
        self.connection
            .query_row(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        aliases_json,
                        tags_json,
                        refs_json,
                        todo_keyword,
                        scheduled_for,
                        deadline_for,
                        closed_at,
                        level,
                        line,
                        kind
                   FROM nodes
                  WHERE explicit_id = ?1
                  LIMIT 1",
                params![explicit_id],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node from ID")
    }

    pub fn node_from_title_or_alias(
        &self,
        title_or_alias: &str,
        nocase: bool,
    ) -> Result<Vec<NodeRecord>> {
        let sql = if nocase {
            "SELECT DISTINCT n.node_key,
                             n.explicit_id,
                             n.file_path,
                             n.title,
                             n.outline_path,
                             n.aliases_json,
                             n.tags_json,
                             n.refs_json,
                             n.todo_keyword,
                             n.scheduled_for,
                             n.deadline_for,
                             n.closed_at,
                             n.level,
                             n.line,
                             n.kind
               FROM nodes AS n
               LEFT JOIN aliases AS a ON a.node_key = n.node_key
              WHERE n.title = ?1 COLLATE NOCASE
                 OR a.alias = ?1 COLLATE NOCASE
              ORDER BY n.file_path, n.line
              LIMIT 2"
        } else {
            "SELECT DISTINCT n.node_key,
                             n.explicit_id,
                             n.file_path,
                             n.title,
                             n.outline_path,
                             n.aliases_json,
                             n.tags_json,
                             n.refs_json,
                             n.todo_keyword,
                             n.scheduled_for,
                             n.deadline_for,
                             n.closed_at,
                             n.level,
                             n.line,
                             n.kind
               FROM nodes AS n
               LEFT JOIN aliases AS a ON a.node_key = n.node_key
              WHERE n.title = ?1
                 OR a.alias = ?1
              ORDER BY n.file_path, n.line
              LIMIT 2"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params![title_or_alias], row_to_node)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to fetch node from title or alias")
    }

    pub fn node_at_point(&self, file_path: &str, line: u32) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        aliases_json,
                        tags_json,
                        refs_json,
                        todo_keyword,
                        scheduled_for,
                        deadline_for,
                        closed_at,
                        level,
                        line,
                        kind
                   FROM nodes
                  WHERE file_path = ?1
                    AND line <= ?2
                  ORDER BY line DESC, level DESC
                  LIMIT 1",
                params![file_path, line],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node at point")
    }

    pub fn node_by_key(&self, node_key: &str) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        aliases_json,
                        tags_json,
                        refs_json,
                        todo_keyword,
                        scheduled_for,
                        deadline_for,
                        closed_at,
                        level,
                        line,
                        kind
                   FROM nodes
                  WHERE node_key = ?1",
                params![node_key],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node by key")
    }
}

pub(crate) fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<NodeRecord> {
    let kind_text: String = row.get(14)?;
    Ok(NodeRecord {
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
    })
}

pub(crate) fn row_to_node_with_offset(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<NodeRecord> {
    let kind_text: String = row.get(offset + 14)?;
    Ok(NodeRecord {
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
    })
}

fn parse_string_list(value: String) -> Vec<String> {
    match serde_json::from_str::<Value>(&value) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(string) if !string.is_empty() => Some(string),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn build_fts_query(input: &str) -> Option<String> {
    let tokens = input
        .split_whitespace()
        .filter_map(|token| {
            let cleaned = token
                .chars()
                .filter(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
                .collect::<String>();
            if cleaned.is_empty() {
                None
            } else {
                Some(format!("{cleaned}*"))
            }
        })
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

fn escape_like_pattern(input: &str) -> String {
    input
        .chars()
        .flat_map(|character| match character {
            '\\' => ['\\', '\\'].into_iter().collect::<Vec<_>>(),
            '%' => ['\\', '%'].into_iter().collect::<Vec<_>>(),
            '_' => ['\\', '_'].into_iter().collect::<Vec<_>>(),
            _ => [character].into_iter().collect::<Vec<_>>(),
        })
        .collect()
}

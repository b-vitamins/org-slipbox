use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{NodeRecord, RefRecord, normalize_reference};

use crate::Database;
use crate::nodes::{row_to_node, row_to_node_with_offset};

impl Database {
    pub fn search_refs(&self, query: &str, limit: usize) -> Result<Vec<RefRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if query.trim().is_empty() {
            let mut statement = self.connection.prepare(
                "SELECT r.ref,
                        n.node_key,
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
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_ref)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read refs")
        } else {
            let query = query.trim();
            let normalized = normalize_reference(query)
                .into_iter()
                .next()
                .unwrap_or_else(|| query.to_owned());
            let bare = query.trim_start_matches('@');
            let mut statement = self.connection.prepare(
                "SELECT r.ref,
                        n.node_key,
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
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE r.ref LIKE ?1
                     OR r.ref LIKE ?2
                     OR r.ref LIKE ?3
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?4",
            )?;
            let rows = statement.query_map(
                params![
                    format!("{query}%"),
                    format!("{normalized}%"),
                    format!("@{bare}%"),
                    limit
                ],
                row_to_ref,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to search refs")
        }
    }

    pub fn node_from_ref(&self, reference: &str) -> Result<Option<NodeRecord>> {
        let normalized = normalize_reference(reference);
        let Some(reference) = normalized.first() else {
            return Ok(None);
        };

        self.connection
            .query_row(
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
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE r.ref = ?1
                  ORDER BY n.file_path, n.line
                  LIMIT 1",
                params![reference],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node from ref")
    }
}

fn row_to_ref(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    Ok(RefRecord {
        reference: row.get(0)?,
        node: row_to_node_with_offset(row, 1)?,
    })
}

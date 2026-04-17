use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{NodeRecord, RefRecord, normalize_reference};

use crate::Database;
use crate::nodes::{anchor_select_columns, row_to_note, row_to_note_with_offset};

impl Database {
    pub fn search_refs(&self, query: &str, limit: usize) -> Result<Vec<RefRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if query.trim().is_empty() {
            let sql = format!(
                "SELECT r.ref,
                        {}
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE {}
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?1",
                anchor_select_columns("n"),
                crate::nodes::note_where("n")
            );
            let mut statement = self.connection.prepare(&sql)?;
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
            let sql = format!(
                "SELECT r.ref,
                        {}
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE {}
                    AND (r.ref LIKE ?1
                     OR r.ref LIKE ?2
                     OR r.ref LIKE ?3)
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?4",
                anchor_select_columns("n"),
                crate::nodes::note_where("n")
            );
            let mut statement = self.connection.prepare(&sql)?;
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

        let sql = format!(
            "SELECT {}
               FROM refs AS r
               JOIN nodes AS n ON n.node_key = r.node_key
              WHERE {}
                AND r.ref = ?1
              ORDER BY n.file_path, n.line
              LIMIT 1",
            anchor_select_columns("n"),
            crate::nodes::note_where("n")
        );
        self.connection
            .query_row(&sql, params![reference], row_to_note)
            .optional()
            .context("failed to fetch node from ref")
    }
}

fn row_to_ref(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    Ok(RefRecord {
        reference: row.get(0)?,
        node: row_to_note_with_offset(row, 1)?,
    })
}

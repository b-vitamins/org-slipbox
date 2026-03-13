use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::BacklinkRecord;

use crate::Database;
use crate::nodes::{NODE_SELECT_COLUMN_COUNT, node_select_columns, row_to_node_with_offset};

impl Database {
    pub fn backlinks(
        &self,
        node_key: &str,
        limit: usize,
        unique: bool,
    ) -> Result<Vec<BacklinkRecord>> {
        let explicit_id = self
            .connection
            .query_row(
                "SELECT explicit_id
                   FROM nodes
                  WHERE node_key = ?1",
                params![node_key],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .context("failed to resolve node for backlink lookup")?
            .flatten();

        let Some(explicit_id) = explicit_id else {
            return Ok(Vec::new());
        };

        let mut statement = if unique {
            let sql = format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM (
                         SELECT source_node_key,
                                line,
                                column,
                                preview,
                                ROW_NUMBER() OVER (
                                  PARTITION BY source_node_key
                                  ORDER BY line, column
                                ) AS occurrence_rank
                           FROM links
                          WHERE destination_explicit_id = ?1
                        ) AS l
                   JOIN nodes AS n ON n.node_key = l.source_node_key
                  WHERE l.occurrence_rank = 1
                  ORDER BY n.file_path, l.line, l.column
                  LIMIT ?2",
                node_select_columns("n")
            );
            self.connection.prepare(&sql)?
        } else {
            let sql = format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS n ON n.node_key = l.source_node_key
                  WHERE l.destination_explicit_id = ?1
                  ORDER BY n.file_path, l.line, l.column
                  LIMIT ?2",
                node_select_columns("n")
            );
            self.connection.prepare(&sql)?
        };
        let rows = statement.query_map(
            params![explicit_id, limit.clamp(1, 1_000) as i64],
            row_to_backlink,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read backlinks")
    }
}

fn row_to_backlink(row: &rusqlite::Row<'_>) -> rusqlite::Result<BacklinkRecord> {
    Ok(BacklinkRecord {
        source_node: row_to_node_with_offset(row, 0)?,
        row: row.get(NODE_SELECT_COLUMN_COUNT)?,
        col: row.get(NODE_SELECT_COLUMN_COUNT + 1)?,
        preview: row.get(NODE_SELECT_COLUMN_COUNT + 2)?,
    })
}

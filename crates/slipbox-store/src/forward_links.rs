use anyhow::{Context, Result};
use rusqlite::params;

use slipbox_core::ForwardLinkRecord;

use crate::Database;
use crate::nodes::{NODE_SELECT_COLUMN_COUNT, node_select_columns, row_to_node_with_offset};

impl Database {
    pub fn forward_links(
        &self,
        node_key: &str,
        limit: usize,
        unique: bool,
    ) -> Result<Vec<ForwardLinkRecord>> {
        let mut statement = if unique {
            let sql = format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM (
                         SELECT dest.node_key AS destination_node_key,
                                l.line,
                                l.column,
                                l.preview,
                                ROW_NUMBER() OVER (
                                  PARTITION BY dest.node_key
                                  ORDER BY l.line, l.column
                                ) AS occurrence_rank
                           FROM links AS l
                           JOIN nodes AS dest
                             ON dest.explicit_id = l.destination_explicit_id
                          WHERE l.source_node_key = ?1
                        ) AS l
                   JOIN nodes AS dest ON dest.node_key = l.destination_node_key
                  WHERE l.occurrence_rank = 1
                  ORDER BY l.line, l.column, dest.file_path, dest.line
                  LIMIT ?2",
                node_select_columns("dest")
            );
            self.connection.prepare(&sql)?
        } else {
            let sql = format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
                  WHERE l.source_node_key = ?1
                  ORDER BY l.line, l.column, dest.file_path, dest.line
                  LIMIT ?2",
                node_select_columns("dest")
            );
            self.connection.prepare(&sql)?
        };
        let rows = statement.query_map(
            params![node_key, limit.clamp(1, 1_000) as i64],
            row_to_forward_link,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read forward links")
    }
}

fn row_to_forward_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<ForwardLinkRecord> {
    Ok(ForwardLinkRecord {
        destination_node: row_to_node_with_offset(row, 0)?,
        row: row.get(NODE_SELECT_COLUMN_COUNT)?,
        col: row.get(NODE_SELECT_COLUMN_COUNT + 1)?,
        preview: row.get(NODE_SELECT_COLUMN_COUNT + 2)?,
    })
}

use anyhow::{Context, Result};
use rusqlite::params;

use slipbox_core::IndexedLink;

use crate::Database;

impl Database {
    pub fn links_to_destination_in_file(
        &self,
        file_path: &str,
        destination_explicit_id: &str,
    ) -> Result<Vec<IndexedLink>> {
        let mut statement = self.connection.prepare(
            "SELECT l.source_node_key,
                    l.destination_explicit_id,
                    l.line,
                    l.column,
                    l.preview
               FROM links AS l
               JOIN nodes AS n ON n.node_key = l.source_node_key
              WHERE n.file_path = ?1
                AND l.destination_explicit_id = ?2
              ORDER BY l.line, l.column",
        )?;
        let rows = statement.query_map(params![file_path, destination_explicit_id], |row| {
            Ok(IndexedLink {
                source_node_key: row.get(0)?,
                destination_explicit_id: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                preview: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read indexed links for file")
    }
}

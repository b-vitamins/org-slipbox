use std::collections::HashMap;

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
              WHERE l.source_file_path = ?1
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

    pub fn links_to_destination_by_file(
        &self,
        destination_explicit_id: &str,
    ) -> Result<HashMap<String, Vec<IndexedLink>>> {
        let mut statement = self.connection.prepare(
            "SELECT l.source_file_path,
                    l.source_node_key,
                    l.destination_explicit_id,
                    l.line,
                    l.column,
                    l.preview
               FROM links AS l
              WHERE l.destination_explicit_id = ?1
              ORDER BY l.source_file_path, l.line, l.column",
        )?;
        let rows = statement.query_map(params![destination_explicit_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                IndexedLink {
                    source_node_key: row.get(1)?,
                    destination_explicit_id: row.get(2)?,
                    line: row.get(3)?,
                    column: row.get(4)?,
                    preview: row.get(5)?,
                },
            ))
        })?;

        let mut grouped = HashMap::<String, Vec<IndexedLink>>::new();
        for row in rows {
            let (file_path, link) = row?;
            grouped.entry(file_path).or_default().push(link);
        }
        Ok(grouped)
    }
}

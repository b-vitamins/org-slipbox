use anyhow::{Context, Result};
use rusqlite::params;

use slipbox_core::FileRecord;

use crate::Database;

impl Database {
    pub fn search_files(&self, query: &str, limit: usize) -> Result<Vec<FileRecord>> {
        let normalized_query = query.trim();
        let mut statement = self.connection.prepare(
            "SELECT f.path,
                    f.title,
                    f.mtime_ns,
                    COALESCE(
                      (SELECT COUNT(*)
                         FROM nodes AS count_nodes
                        WHERE count_nodes.file_path = f.path),
                      0
                    ) AS node_count
               FROM files AS f
              WHERE (?1 = ''
                     OR instr(lower(f.path), lower(?1)) > 0
                     OR instr(lower(f.title), lower(?1)) > 0)
              ORDER BY f.path COLLATE NOCASE, f.path
              LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![normalized_query, limit.clamp(1, 200) as i64],
            row_to_file_record,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read indexed file records")
    }
}

fn row_to_file_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        file_path: row.get(0)?,
        title: row.get(1)?,
        mtime_ns: row.get(2)?,
        node_count: row.get(3)?,
    })
}

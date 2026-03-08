use anyhow::{Context, Result};

use slipbox_core::IndexStats;

use crate::Database;

impl Database {
    pub fn stats(&self) -> Result<IndexStats> {
        Ok(IndexStats {
            files_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM files", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed files")?,
            nodes_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed nodes")?,
            links_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM links", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed links")?,
        })
    }

    pub fn indexed_files(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM files ORDER BY path")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read indexed file list")
    }
}

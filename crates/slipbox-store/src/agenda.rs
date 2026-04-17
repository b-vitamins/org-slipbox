use anyhow::{Context, Result};
use rusqlite::params;
use slipbox_core::AnchorRecord;

use crate::Database;
use crate::nodes::{anchor_select_columns, row_to_anchor};

impl Database {
    pub fn agenda_nodes(&self, start: &str, end: &str, limit: usize) -> Result<Vec<AnchorRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE (n.scheduled_for IS NOT NULL AND n.scheduled_for >= ?1 AND n.scheduled_for <= ?2)
                 OR (n.deadline_for IS NOT NULL AND n.deadline_for >= ?1 AND n.deadline_for <= ?2)
              ORDER BY COALESCE(n.scheduled_for, n.deadline_for), n.file_path, n.line
              LIMIT ?3",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![start, end, limit.clamp(1, 500) as i64],
            row_to_anchor,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read agenda nodes")
    }
}

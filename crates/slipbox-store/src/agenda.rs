use anyhow::{Context, Result};
use rusqlite::params;
use slipbox_core::NodeRecord;

use crate::Database;
use crate::nodes::row_to_node;

impl Database {
    pub fn agenda_nodes(&self, start: &str, end: &str, limit: usize) -> Result<Vec<NodeRecord>> {
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
              WHERE (scheduled_for IS NOT NULL AND scheduled_for >= ?1 AND scheduled_for <= ?2)
                 OR (deadline_for IS NOT NULL AND deadline_for >= ?1 AND deadline_for <= ?2)
              ORDER BY COALESCE(scheduled_for, deadline_for), file_path, line
              LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![start, end, limit.clamp(1, 500) as i64], row_to_node)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read agenda nodes")
    }
}

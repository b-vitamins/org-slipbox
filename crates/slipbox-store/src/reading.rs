use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use crate::Database;

impl Database {
    /// Retain source-proven file moves so a device-local reading reference can
    /// follow a file note after its derived node key changes. Chained moves are
    /// collapsed to their current path; aliases whose destination disappeared
    /// are discarded rather than resolving to an unrelated note.
    pub fn record_reading_file_renames(&mut self, renames: &[(String, String)]) -> Result<()> {
        let transaction = self.connection.transaction()?;
        for (from, to) in renames {
            transaction.execute(
                "UPDATE reading_file_renames
                    SET current_file_path = ?2
                  WHERE current_file_path = ?1",
                params![from, to],
            )?;
            transaction.execute(
                "INSERT INTO reading_file_renames (previous_file_path, current_file_path)
                 VALUES (?1, ?2)
                 ON CONFLICT(previous_file_path) DO UPDATE
                     SET current_file_path = excluded.current_file_path",
                params![from, to],
            )?;
        }
        transaction.execute(
            "DELETE FROM reading_file_renames
              WHERE previous_file_path = current_file_path
                 OR current_file_path NOT IN (SELECT path FROM files)",
            [],
        )?;
        transaction
            .commit()
            .context("failed to record reading file renames")
    }

    pub(crate) fn current_reading_file_path(&self, previous: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT current_file_path
                   FROM reading_file_renames
                  WHERE previous_file_path = ?1",
                [previous],
                |row| row.get(0),
            )
            .optional()
            .context("failed to resolve a reading file rename")
    }
}

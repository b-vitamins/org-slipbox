use anyhow::{Context, Result};
use rusqlite::params;

use slipbox_core::{AnchorExplorationRecord, AnchorRecord, ExplorationExplanation};

use crate::Database;
use crate::nodes::{ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, row_to_anchor};

impl Database {
    pub fn time_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let scheduled_for = anchor.scheduled_for.as_deref();
        let deadline_for = anchor.deadline_for.as_deref();
        if scheduled_for.is_none() && deadline_for.is_none() {
            return Ok(Vec::new());
        }

        let sql = format!(
            "SELECT {},
                    CASE
                        WHEN ?2 IS NOT NULL AND n.scheduled_for = ?2 THEN 'scheduled'
                        WHEN ?3 IS NOT NULL AND n.deadline_for = ?3 THEN 'deadline'
                    END AS match_kind,
                    CASE
                        WHEN ?2 IS NOT NULL AND n.scheduled_for = ?2 THEN n.scheduled_for
                        WHEN ?3 IS NOT NULL AND n.deadline_for = ?3 THEN n.deadline_for
                    END AS match_date
               FROM nodes AS n
              WHERE n.node_key <> ?1
                AND ((?2 IS NOT NULL AND n.scheduled_for = ?2)
                  OR (?3 IS NOT NULL AND n.deadline_for = ?3))
              ORDER BY COALESCE(n.scheduled_for, n.deadline_for), n.file_path, n.line
              LIMIT ?4",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![
                anchor.node_key,
                scheduled_for,
                deadline_for,
                limit.clamp(1, 1_000) as i64
            ],
            row_to_time_neighbor,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read time neighbors")
    }

    pub fn task_neighbors(
        &self,
        anchor: &AnchorRecord,
        limit: usize,
    ) -> Result<Vec<AnchorExplorationRecord>> {
        let Some(todo_keyword) = anchor.todo_keyword.as_deref() else {
            return Ok(Vec::new());
        };

        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE n.todo_keyword = ?1
                AND n.node_key <> ?2
              ORDER BY n.file_path, n.line
              LIMIT ?3",
            anchor_select_columns("n")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(
            params![todo_keyword, anchor.node_key, limit.clamp(1, 1_000) as i64],
            |row| {
                Ok(AnchorExplorationRecord {
                    anchor: row_to_anchor(row)?,
                    explanation: ExplorationExplanation::SharedTodoKeyword {
                        todo_keyword: todo_keyword.to_owned(),
                    },
                })
            },
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read task neighbors")
    }
}

fn row_to_time_neighbor(row: &rusqlite::Row<'_>) -> rusqlite::Result<AnchorExplorationRecord> {
    let match_kind: String = row.get(ANCHOR_SELECT_COLUMN_COUNT)?;
    let match_date: String = row.get(ANCHOR_SELECT_COLUMN_COUNT + 1)?;
    let explanation = match match_kind.as_str() {
        "scheduled" => ExplorationExplanation::SharedScheduledDate { date: match_date },
        "deadline" => ExplorationExplanation::SharedDeadlineDate { date: match_date },
        other => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                ANCHOR_SELECT_COLUMN_COUNT,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(format!(
                    "unexpected time-neighbor match kind {other}"
                ))),
            ));
        }
    };
    Ok(AnchorExplorationRecord {
        anchor: row_to_anchor(row)?,
        explanation,
    })
}

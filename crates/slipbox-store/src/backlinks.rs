use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{AnchorRecord, BacklinkRecord, ExplorationExplanation, NodeRecord};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_where, row_to_anchor_with_offset,
    row_to_note_with_offset,
};

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
            .context("failed to resolve note for backlink lookup")?
            .flatten();

        let Some(explicit_id) = explicit_id else {
            return Ok(Vec::new());
        };

        let limit = limit.clamp(1, 1_000);
        let sql = if unique {
            format!(
                "SELECT {},
                        {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS a ON a.node_key = l.source_node_key
                   JOIN nodes AS src ON src.node_key = l.source_note_key
                  WHERE l.destination_explicit_id = ?1
                    AND {}
                  ORDER BY l.source_file_path, l.line, l.column",
                anchor_select_columns("src"),
                anchor_select_columns("a"),
                note_where("src"),
            )
        } else {
            format!(
                "SELECT {},
                        {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS a ON a.node_key = l.source_node_key
                   JOIN nodes AS src ON src.node_key = l.source_note_key
                  WHERE l.destination_explicit_id = ?1
                    AND {}
                  ORDER BY l.source_file_path, l.line, l.column
                  LIMIT ?2",
                anchor_select_columns("src"),
                anchor_select_columns("a"),
                note_where("src"),
            )
        };
        let mut statement = self.connection.prepare(&sql)?;
        let mut rows = if unique {
            statement.query(params![explicit_id])?
        } else {
            statement.query(params![explicit_id, limit as i64])?
        };
        let mut results = Vec::new();
        let mut seen_notes = HashSet::new();

        while let Some(row) = rows.next().context("failed to read backlink row")? {
            let source = row_to_backlink_source(row).context("failed to decode backlink row")?;
            let source_anchor = if source.anchor.node_key == source.note.node_key {
                None
            } else {
                Some(source.anchor)
            };
            if unique && !seen_notes.insert(source.note.node_key.clone()) {
                continue;
            }
            results.push(BacklinkRecord {
                source_note: source.note,
                source_anchor,
                row: source.row,
                col: source.col,
                preview: source.preview,
                explanation: ExplorationExplanation::Backlink,
            });
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }
}

struct BacklinkSource {
    note: NodeRecord,
    anchor: AnchorRecord,
    row: u32,
    col: u32,
    preview: String,
}

fn row_to_backlink_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<BacklinkSource> {
    Ok(BacklinkSource {
        note: row_to_note_with_offset(row, 0)?,
        anchor: row_to_anchor_with_offset(row, ANCHOR_SELECT_COLUMN_COUNT)?,
        row: row.get(ANCHOR_SELECT_COLUMN_COUNT * 2)?,
        col: row.get(ANCHOR_SELECT_COLUMN_COUNT * 2 + 1)?,
        preview: row.get(ANCHOR_SELECT_COLUMN_COUNT * 2 + 2)?,
    })
}

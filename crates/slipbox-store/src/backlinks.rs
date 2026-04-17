use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{AnchorRecord, BacklinkRecord};

use crate::Database;
use crate::nodes::{ANCHOR_SELECT_COLUMN_COUNT, row_to_anchor_with_offset};

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

        let sql = format!(
            "SELECT {},
                    l.line,
                    l.column,
                    l.preview
               FROM links AS l
               JOIN nodes AS a ON a.node_key = l.source_node_key
              WHERE l.destination_explicit_id = ?1
              ORDER BY a.file_path, l.line, l.column",
            crate::nodes::anchor_select_columns("a")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![explicit_id], row_to_backlink_source)?;
        let sources = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read backlink anchors")?;

        let limit = limit.clamp(1, 1_000);
        let mut results = Vec::new();
        let mut seen_notes = HashSet::new();

        for source in sources {
            let source_note = self.note_for_anchor(&source.anchor)?.ok_or_else(|| {
                anyhow::anyhow!("anchor {} has no owning note", source.anchor.node_key)
            })?;
            if unique && !seen_notes.insert(source_note.node_key.clone()) {
                continue;
            }
            let source_anchor =
                (source.anchor.node_key != source_note.node_key).then_some(source.anchor);
            results.push(BacklinkRecord {
                source_note,
                source_anchor,
                row: source.row,
                col: source.col,
                preview: source.preview,
            });
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }
}

struct BacklinkSource {
    anchor: AnchorRecord,
    row: u32,
    col: u32,
    preview: String,
}

fn row_to_backlink_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<BacklinkSource> {
    Ok(BacklinkSource {
        anchor: row_to_anchor_with_offset(row, 0)?,
        row: row.get(ANCHOR_SELECT_COLUMN_COUNT)?,
        col: row.get(ANCHOR_SELECT_COLUMN_COUNT + 1)?,
        preview: row.get(ANCHOR_SELECT_COLUMN_COUNT + 2)?,
    })
}

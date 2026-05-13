use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{AnchorRecord, BacklinkRecord, ExplorationExplanation, NodeRecord};

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

        let limit = limit.clamp(1, 1_000);
        let sql = if unique {
            format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS a ON a.node_key = l.source_node_key
                  WHERE l.destination_explicit_id = ?1
                  ORDER BY l.source_file_path, l.line, l.column",
                crate::nodes::anchor_select_columns("a")
            )
        } else {
            format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS a ON a.node_key = l.source_node_key
                  WHERE l.destination_explicit_id = ?1
                  ORDER BY l.source_file_path, l.line, l.column
                  LIMIT ?2",
                crate::nodes::anchor_select_columns("a")
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
        let mut note_owner_cache: HashMap<String, HashMap<String, NodeRecord>> = HashMap::new();

        while let Some(row) = rows.next().context("failed to read backlink row")? {
            let source = row_to_backlink_source(row).context("failed to decode backlink row")?;
            let (source_note, source_anchor) = if source.anchor.is_note() {
                (
                    NodeRecord::try_from(source.anchor.clone()).map_err(|_| {
                        anyhow::anyhow!("anchor {} is not a canonical note", source.anchor.node_key)
                    })?,
                    None,
                )
            } else {
                if !note_owner_cache.contains_key(&source.anchor.file_path) {
                    let owners = self
                        .note_owners_by_anchor_key(&source.anchor.file_path)
                        .with_context(|| {
                            format!(
                                "failed to resolve note owners for {}",
                                source.anchor.file_path
                            )
                        })?;
                    note_owner_cache.insert(source.anchor.file_path.clone(), owners);
                }
                let source_note = note_owner_cache
                    .get(&source.anchor.file_path)
                    .and_then(|owners| owners.get(&source.anchor.node_key))
                    .cloned()
                    .ok_or_else(|| {
                        anyhow::anyhow!("anchor {} has no owning note", source.anchor.node_key)
                    })?;
                (source_note, Some(source.anchor))
            };
            if unique && !seen_notes.insert(source_note.node_key.clone()) {
                continue;
            }
            results.push(BacklinkRecord {
                source_note,
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

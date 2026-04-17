use anyhow::{Context, Result};
use rusqlite::params_from_iter;

use slipbox_core::ForwardLinkRecord;

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_owners_by_anchor_key, note_where,
    row_to_note_with_offset,
};

impl Database {
    pub fn forward_links(
        &self,
        node_key: &str,
        limit: usize,
        unique: bool,
    ) -> Result<Vec<ForwardLinkRecord>> {
        let Some(source_note) = self.note_by_key(node_key)? else {
            return Ok(Vec::new());
        };
        let anchors = self.anchors_in_file(&source_note.file_path)?;
        let owners = note_owners_by_anchor_key(&anchors);
        let source_anchor_keys = owners
            .into_iter()
            .filter_map(|(anchor_key, owner)| {
                (owner.node_key == source_note.node_key).then_some(anchor_key)
            })
            .collect::<Vec<_>>();
        if source_anchor_keys.is_empty() {
            return Ok(Vec::new());
        }

        let limit = limit.clamp(1, 1_000);
        let placeholders = (1..=source_anchor_keys.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let limit_placeholder = source_anchor_keys.len() + 1;
        let sql = if unique {
            format!(
                "SELECT {},
                        matches.line,
                        matches.column,
                        matches.preview
                   FROM (
                         SELECT dest.node_key AS destination_note_key,
                                l.line,
                                l.column,
                                l.preview,
                                ROW_NUMBER() OVER (
                                    PARTITION BY dest.node_key
                                    ORDER BY l.line, l.column, dest.file_path, dest.line
                                ) AS occurrence_rank
                           FROM links AS l
                           JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
                          WHERE l.source_node_key IN ({})
                            AND {}
                        ) AS matches
                   JOIN nodes AS dest ON dest.node_key = matches.destination_note_key
                  WHERE matches.occurrence_rank = 1
                  ORDER BY matches.line, matches.column, dest.file_path, dest.line
                  LIMIT ?{}",
                anchor_select_columns("dest"),
                placeholders,
                note_where("dest"),
                limit_placeholder,
            )
        } else {
            format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
                  WHERE l.source_node_key IN ({})
                    AND {}
                  ORDER BY l.line, l.column, dest.file_path, dest.line
                  LIMIT ?{}",
                anchor_select_columns("dest"),
                placeholders,
                note_where("dest"),
                limit_placeholder,
            )
        };
        let mut values = source_anchor_keys;
        values.push(limit.to_string());
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values.iter()), row_to_forward_link)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read forward links")
    }
}

fn row_to_forward_link(row: &rusqlite::Row<'_>) -> rusqlite::Result<ForwardLinkRecord> {
    Ok(ForwardLinkRecord {
        destination_note: row_to_note_with_offset(row, 0)?,
        row: row.get(ANCHOR_SELECT_COLUMN_COUNT)?,
        col: row.get(ANCHOR_SELECT_COLUMN_COUNT + 1)?,
        preview: row.get(ANCHOR_SELECT_COLUMN_COUNT + 2)?,
    })
}

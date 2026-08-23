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
        let Some(explicit_id) = self.backlink_destination_id(node_key)? else {
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

    /// Count distinct backlink source notes.
    pub fn backlink_note_count(&self, node_key: &str) -> Result<u64> {
        let Some(explicit_id) = self.backlink_destination_id(node_key)? else {
            return Ok(0);
        };
        let sql = format!(
            "SELECT COUNT(DISTINCT l.source_note_key)
               FROM links AS l
               JOIN nodes AS a ON a.node_key = l.source_node_key
               JOIN nodes AS src ON src.node_key = l.source_note_key
              WHERE l.destination_explicit_id = ?1
                AND {}",
            note_where("src"),
        );
        self.connection
            .query_row(&sql, params![explicit_id], |row| row.get::<_, u64>(0))
            .context("failed to count backlink notes")
    }

    fn backlink_destination_id(&self, node_key: &str) -> Result<Option<String>> {
        self.connection
            .query_row(
                "SELECT explicit_id
                   FROM nodes
                  WHERE node_key = ?1",
                params![node_key],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .context("failed to resolve note for backlink lookup")
            .map(Option::flatten)
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

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::test_support::indexed_database;

    #[test]
    fn backlink_note_count_counts_notes_where_the_stored_column_counts_links() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "alpha.org",
                r#":PROPERTIES:
:ID: alpha-id
:END:
#+title: Alpha
"#,
            ),
            (
                "beta.org",
                r#":PROPERTIES:
:ID: beta-id
:END:
#+title: Beta

See [[id:alpha-id][Alpha]].
And again, [[id:alpha-id][Alpha]].
"#,
            ),
            (
                "gamma.org",
                r#":PROPERTIES:
:ID: gamma-id
:END:
#+title: Gamma

See [[id:alpha-id][Alpha]].
"#,
            ),
        ])?;

        let alpha = database
            .node_from_id("alpha-id")?
            .expect("alpha should be indexed");

        assert_eq!(
            alpha.backlink_count, 3,
            "the stored column sums link rows, and beta links twice",
        );
        assert_eq!(database.backlink_note_count(&alpha.node_key)?, 2);
        assert_eq!(database.backlinks(&alpha.node_key, 25, true)?.len(), 2);
        Ok(())
    }

    #[test]
    fn backlink_note_count_is_zero_for_a_note_nothing_links_to() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[(
            "alpha.org",
            r#":PROPERTIES:
:ID: alpha-id
:END:
#+title: Alpha
"#,
        )])?;

        let alpha = database
            .node_from_id("alpha-id")?
            .expect("alpha should be indexed");

        assert_eq!(database.backlink_note_count(&alpha.node_key)?, 0);
        Ok(())
    }
}

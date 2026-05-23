use anyhow::{Context, Result};
use rusqlite::{OptionalExtension, params};

use slipbox_core::{ExplorationExplanation, ForwardLinkRecord};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_where, row_to_note_with_offset,
};

impl Database {
    pub fn forward_links(
        &self,
        node_key: &str,
        limit: usize,
        unique: bool,
    ) -> Result<Vec<ForwardLinkRecord>> {
        let Some(source_note_key) = self
            .connection
            .query_row(
                &format!(
                    "SELECT n.node_key
                       FROM nodes AS n
                      WHERE n.node_key = ?1
                        AND {}",
                    note_where("n"),
                ),
                params![node_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to resolve source note for forward links")?
        else {
            return Ok(Vec::new());
        };

        let limit = limit.clamp(1, 1_000);
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
                                    ORDER BY l.line, l.column, l.destination_explicit_id
                                ) AS occurrence_rank
                           FROM links AS l
                           JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
                          WHERE l.source_note_key = ?1
                            AND {}
                        ) AS matches
                   JOIN nodes AS dest ON dest.node_key = matches.destination_note_key
                  WHERE matches.occurrence_rank = 1
                  ORDER BY matches.line, matches.column, dest.explicit_id
                  LIMIT ?2",
                anchor_select_columns("dest"),
                note_where("dest"),
            )
        } else {
            format!(
                "SELECT {},
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS dest ON dest.explicit_id = l.destination_explicit_id
                  WHERE l.source_note_key = ?1
                    AND {}
                  ORDER BY l.line, l.column, l.destination_explicit_id
                  LIMIT ?2",
                anchor_select_columns("dest"),
                note_where("dest"),
            )
        };
        let mut statement = self.connection.prepare(&sql)?;
        let rows =
            statement.query_map(params![source_note_key, limit as i64], row_to_forward_link)?;
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
        explanation: ExplorationExplanation::ForwardLink,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::test_support::indexed_database;

    #[test]
    fn forward_links_include_links_owned_by_anonymous_child_anchors() -> Result<()> {
        let (_workspace, database, _root) = indexed_database(&[
            (
                "alpha.org",
                r#":PROPERTIES:
:ID: alpha-id
:END:
#+title: Alpha

* Scratch
See [[id:beta-id][Beta]].
"#,
            ),
            (
                "beta.org",
                r#":PROPERTIES:
:ID: beta-id
:END:
#+title: Beta
"#,
            ),
        ])?;

        let alpha = database
            .node_from_id("alpha-id")?
            .expect("alpha should be indexed");
        let forward_links = database.forward_links(&alpha.node_key, 10, false)?;

        assert_eq!(forward_links.len(), 1);
        assert_eq!(forward_links[0].destination_note.title, "Beta");
        assert_eq!(forward_links[0].row, 7);
        Ok(())
    }
}

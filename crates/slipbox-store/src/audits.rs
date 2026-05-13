use anyhow::{Context, Result};
use rusqlite::params;

use slipbox_core::{
    DanglingLinkAuditRecord, DuplicateTitleAuditRecord, NoteConnectivityAuditRecord,
};

use crate::Database;
use crate::nodes::{
    ANCHOR_SELECT_COLUMN_COUNT, anchor_select_columns, note_where, row_to_anchor_with_offset,
    row_to_note,
};

impl Database {
    pub fn audit_dangling_links(&self, limit: usize) -> Result<Vec<DanglingLinkAuditRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        let sql = format!(
            "SELECT {},
                    link.destination_explicit_id,
                    link.line,
                    link.column,
                    link.preview
               FROM links AS link
               JOIN nodes AS src ON src.node_key = link.source_node_key
              WHERE NOT EXISTS (
                    SELECT 1
                      FROM nodes AS dest
                     WHERE dest.explicit_id = link.destination_explicit_id
                  )
              ORDER BY link.source_file_path, link.line, link.column, link.source_node_key
              LIMIT ?1",
            anchor_select_columns("src")
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(DanglingLinkAuditRecord {
                source: row_to_anchor_with_offset(row, 0)?,
                missing_explicit_id: row.get(ANCHOR_SELECT_COLUMN_COUNT)?,
                line: row.get(ANCHOR_SELECT_COLUMN_COUNT + 1)?,
                column: row.get(ANCHOR_SELECT_COLUMN_COUNT + 2)?,
                preview: row.get(ANCHOR_SELECT_COLUMN_COUNT + 3)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to query dangling link audit")
    }

    pub fn audit_duplicate_titles(&self, limit: usize) -> Result<Vec<DuplicateTitleAuditRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        let duplicate_titles = self.duplicate_title_keys(limit)?;
        let mut records = Vec::with_capacity(duplicate_titles.len());
        for title in duplicate_titles {
            let notes = self.notes_for_duplicate_title(&title)?;
            if notes.len() > 1 {
                records.push(DuplicateTitleAuditRecord { title, notes });
            }
        }
        Ok(records)
    }

    pub fn audit_orphan_notes(&self, limit: usize) -> Result<Vec<NoteConnectivityAuditRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
                AND n.refs_json = '[]'
                AND {} = 0
                AND {} = 0
              ORDER BY n.file_path, n.line
              LIMIT ?1",
            anchor_select_columns("n"),
            note_where("n"),
            backlink_count_sql("n"),
            outgoing_link_count_any_sql("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit], |row| {
            let note = row_to_note(row)?;
            Ok(NoteConnectivityAuditRecord {
                reference_count: note.refs.len(),
                backlink_count: note.backlink_count as usize,
                forward_link_count: note.forward_link_count as usize,
                note,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to query orphan note audit")
    }

    pub fn audit_weakly_integrated_notes(
        &self,
        limit: usize,
    ) -> Result<Vec<NoteConnectivityAuditRecord>> {
        let limit = limit.clamp(1, 500) as i64;
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
                AND n.refs_json <> '[]'
                AND ({} + {}) <= 1
              ORDER BY ({} + {}), n.file_path, n.line
              LIMIT ?1",
            anchor_select_columns("n"),
            note_where("n"),
            backlink_count_sql("n"),
            forward_link_count_sql("n"),
            backlink_count_sql("n"),
            forward_link_count_sql("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit], |row| {
            let note = row_to_note(row)?;
            Ok(NoteConnectivityAuditRecord {
                reference_count: note.refs.len(),
                backlink_count: note.backlink_count as usize,
                forward_link_count: note.forward_link_count as usize,
                note,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to query weakly integrated note audit")
    }

    fn duplicate_title_keys(&self, limit: i64) -> Result<Vec<String>> {
        let sql = format!(
            "SELECT MIN(n.title)
               FROM nodes AS n
              WHERE {}
              GROUP BY n.title COLLATE NOCASE
             HAVING COUNT(*) > 1
              ORDER BY MIN(n.title) COLLATE NOCASE
              LIMIT ?1",
            note_where("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![limit], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to query duplicate title groups")
    }

    fn notes_for_duplicate_title(&self, title: &str) -> Result<Vec<slipbox_core::NodeRecord>> {
        let sql = format!(
            "SELECT {}
               FROM nodes AS n
              WHERE {}
                AND n.title = ?1 COLLATE NOCASE
              ORDER BY n.title COLLATE NOCASE, n.file_path, n.line",
            anchor_select_columns("n"),
            note_where("n"),
        );
        let mut statement = self.connection.prepare(&sql)?;
        let rows = statement.query_map(params![title], row_to_note)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read duplicate title notes")
    }
}

fn backlink_count_sql(alias: &str) -> String {
    format!(
        "COALESCE((SELECT COUNT(*)
                    FROM links AS incoming
                   WHERE incoming.destination_explicit_id = {alias}.explicit_id), 0)"
    )
}

fn forward_link_count_sql(alias: &str) -> String {
    format!(
        "COALESCE((SELECT COUNT(*)
                    FROM links AS outgoing
                    JOIN nodes AS dest ON dest.explicit_id = outgoing.destination_explicit_id
                   WHERE outgoing.source_node_key = {alias}.node_key), 0)"
    )
}

fn outgoing_link_count_any_sql(alias: &str) -> String {
    format!(
        "COALESCE((SELECT COUNT(*)
                    FROM links AS outgoing
                   WHERE outgoing.source_node_key = {alias}.node_key), 0)"
    )
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::test_support::indexed_database;

    fn audit_fixture() -> Result<(tempfile::TempDir, Database)> {
        let (workspace, database, _root) = indexed_database(&[
            (
                "duplicate-a.org",
                r#":PROPERTIES:
:ID: dup-a-id
:END:
#+title: Shared Title

Links to [[id:dup-b-id][Other duplicate]].
"#,
            ),
            (
                "duplicate-b.org",
                r#":PROPERTIES:
:ID: dup-b-id
:END:
#+title: shared title

Links to [[id:dup-a-id][Other duplicate]].
"#,
            ),
            (
                "dangling-source.org",
                r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
            ),
            (
                "orphan.org",
                r#":PROPERTIES:
:ID: orphan-id
:END:
#+title: Orphan

Just an orphan note.
"#,
            ),
            (
                "weak.org",
                r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:weak2024
:END:
#+title: Weak

Has refs but no structural links.
"#,
            ),
        ])?;
        Ok((workspace, database))
    }

    use crate::Database;

    #[test]
    fn audit_dangling_links_returns_source_and_missing_target() -> Result<()> {
        let (_workspace, database) = audit_fixture()?;

        let records = database.audit_dangling_links(20)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].source.title, "Dangling Source");
        assert_eq!(records[0].missing_explicit_id, "missing-id");
        assert_eq!(records[0].line, 6);
        assert!(records[0].preview.contains("Missing"));

        Ok(())
    }

    #[test]
    fn audit_duplicate_titles_groups_case_insensitive_note_titles() -> Result<()> {
        let (_workspace, database) = audit_fixture()?;

        let records = database.audit_duplicate_titles(20)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "Shared Title");
        assert_eq!(records[0].notes.len(), 2);
        assert_eq!(records[0].notes[0].title, "Shared Title");
        assert_eq!(records[0].notes[1].title, "shared title");

        Ok(())
    }

    #[test]
    fn audit_orphan_notes_returns_disconnected_refless_notes() -> Result<()> {
        let (_workspace, database) = audit_fixture()?;

        let records = database.audit_orphan_notes(20)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].note.title, "Orphan");
        assert_eq!(records[0].reference_count, 0);
        assert_eq!(records[0].backlink_count, 0);
        assert_eq!(records[0].forward_link_count, 0);

        Ok(())
    }

    #[test]
    fn audit_weakly_integrated_notes_returns_ref_backed_sparse_notes() -> Result<()> {
        let (_workspace, database) = audit_fixture()?;

        let records = database.audit_weakly_integrated_notes(20)?;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].note.title, "Weak");
        assert_eq!(records[0].reference_count, 1);
        assert_eq!(records[0].backlink_count, 0);
        assert_eq!(records[0].forward_link_count, 0);

        Ok(())
    }
}

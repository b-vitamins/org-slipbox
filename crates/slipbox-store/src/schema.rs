use anyhow::Result;

use crate::Database;

const SCHEMA_VERSION: i32 = 23;

impl Database {
    pub(crate) fn migrate(&self) -> Result<()> {
        let version: i32 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            anyhow::bail!(
                "database schema version {} is newer than supported version {}",
                version,
                SCHEMA_VERSION
            );
        }

        if version < SCHEMA_VERSION {
            self.rebuild_schema()?;
        }

        Ok(())
    }

    fn rebuild_schema(&self) -> Result<()> {
        // The version stamped here is the constant `migrate` compares against; a
        // lower stamp sends every subsequent open back through this rebuild, which
        // drops and recreates the derived tables.
        self.connection.execute_batch(&format!(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;

             DROP TABLE IF EXISTS links;
             DROP TABLE IF EXISTS aliases;
             DROP TABLE IF EXISTS tags;
             DROP TABLE IF EXISTS refs;
             DROP TABLE IF EXISTS occurrence_document_fts;
             DROP TABLE IF EXISTS occurrence_documents;
             DROP TABLE IF EXISTS node_phrase_fts;
             DROP TABLE IF EXISTS node_content_fts;
             DROP TABLE IF EXISTS node_fts;
             DROP TABLE IF EXISTS nodes;
             DROP TABLE IF EXISTS files;

             CREATE TABLE IF NOT EXISTS files (
               path TEXT PRIMARY KEY,
               title TEXT NOT NULL,
               mtime_ns INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS nodes (
               id INTEGER PRIMARY KEY,
               node_key TEXT NOT NULL UNIQUE,
               explicit_id TEXT UNIQUE,
               file_path TEXT NOT NULL,
               title TEXT NOT NULL,
               outline_path TEXT NOT NULL,
               aliases_json TEXT NOT NULL,
               tags_json TEXT NOT NULL,
               refs_json TEXT NOT NULL,
               todo_keyword TEXT,
               scheduled_for TEXT,
               deadline_for TEXT,
               closed_at TEXT,
               glossary INTEGER NOT NULL DEFAULT 0,
               glossary_status TEXT,
               sr_due TEXT,
               sr_ease TEXT,
               sr_interval TEXT,
               sr_reps TEXT,
               sr_last TEXT,
               level INTEGER NOT NULL,
               line INTEGER NOT NULL,
               kind TEXT NOT NULL,
               backlink_count INTEGER NOT NULL DEFAULT 0,
               forward_link_count INTEGER NOT NULL DEFAULT 0
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS node_fts USING fts5(
               title,
               outline_path,
               file_path,
               alias_text,
               ref_text,
               tag_text,
               tokenize='porter unicode61 remove_diacritics 2'
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS node_content_fts USING fts5(
               title,
               aliases,
               body,
               tokenize='porter unicode61 remove_diacritics 2'
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS node_phrase_fts USING fts5(
               text,
               tokenize='unicode61 remove_diacritics 2'
             );

             CREATE TABLE IF NOT EXISTS occurrence_documents (
               id INTEGER PRIMARY KEY,
               file_path TEXT NOT NULL UNIQUE,
               search_text TEXT NOT NULL,
               line_rows_json TEXT NOT NULL
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS occurrence_document_fts USING fts5(
               search_text,
               content='occurrence_documents',
               content_rowid='id',
               tokenize='trigram'
             );

             CREATE TABLE IF NOT EXISTS refs (
               node_key TEXT NOT NULL,
               ref TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS aliases (
               node_key TEXT NOT NULL,
               alias TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS tags (
               node_key TEXT NOT NULL,
               tag TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS links (
               source_node_key TEXT NOT NULL,
               source_note_key TEXT NOT NULL,
               source_file_path TEXT NOT NULL,
               destination_explicit_id TEXT NOT NULL,
               line INTEGER NOT NULL,
               column INTEGER NOT NULL,
               preview TEXT NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_nodes_file_path
               ON nodes (file_path);

             CREATE INDEX IF NOT EXISTS idx_nodes_file_path_line_level
               ON nodes (file_path, line, level);

             CREATE INDEX IF NOT EXISTS idx_nodes_title
               ON nodes (title);

             CREATE INDEX IF NOT EXISTS idx_nodes_title_nocase
               ON nodes (title COLLATE NOCASE);

             CREATE INDEX IF NOT EXISTS idx_occurrence_documents_file_path
               ON occurrence_documents (file_path);

             CREATE INDEX IF NOT EXISTS idx_nodes_explicit_id
               ON nodes (explicit_id)
               WHERE explicit_id IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_links_source_node_key
               ON links (source_node_key);

             CREATE INDEX IF NOT EXISTS idx_links_destination_explicit_id
               ON links (destination_explicit_id);

             CREATE INDEX IF NOT EXISTS idx_links_destination_source_file_line
               ON links (destination_explicit_id, source_file_path, line, column, source_note_key, source_node_key);

             CREATE INDEX IF NOT EXISTS idx_links_source_file_line
               ON links (source_file_path, line, column, source_node_key);

             CREATE INDEX IF NOT EXISTS idx_refs_ref
               ON refs (ref);

             CREATE INDEX IF NOT EXISTS idx_refs_node_key
               ON refs (node_key);

             CREATE INDEX IF NOT EXISTS idx_aliases_alias
               ON aliases (alias);

             CREATE INDEX IF NOT EXISTS idx_aliases_node_key
               ON aliases (node_key);

             CREATE INDEX IF NOT EXISTS idx_aliases_alias_nocase
               ON aliases (alias COLLATE NOCASE);

             CREATE INDEX IF NOT EXISTS idx_tags_tag
               ON tags (tag);

             CREATE INDEX IF NOT EXISTS idx_tags_node_key
               ON tags (node_key);

             CREATE INDEX IF NOT EXISTS idx_nodes_scheduled_for
               ON nodes (scheduled_for)
               WHERE scheduled_for IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_nodes_deadline_for
               ON nodes (deadline_for)
               WHERE deadline_for IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_nodes_todo_keyword
               ON nodes (todo_keyword)
               WHERE todo_keyword IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_links_source_note_destination
               ON links (source_note_key, destination_explicit_id);

             CREATE INDEX IF NOT EXISTS idx_links_source_note_line
               ON links (source_note_key, line, column, destination_explicit_id);

             CREATE INDEX IF NOT EXISTS idx_nodes_glossary
               ON nodes (title COLLATE NOCASE, file_path, line)
               WHERE glossary = 1;

             CREATE INDEX IF NOT EXISTS idx_nodes_sr_due
               ON nodes (sr_due)
               WHERE glossary = 1;

             PRAGMA user_version = {SCHEMA_VERSION};"
        ))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::{Context, Result};
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};

    use crate::Database;

    fn index_fixture(body: &str) -> Result<Database> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        fs::write(root.join("term.org"), body).context("fixture should be written")?;
        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        Ok(database)
    }

    #[test]
    fn a_rebuilt_database_opens_without_rebuilding_again() -> Result<()> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let path = workspace.path().join("index.sqlite3");
        Database::open(&path)?;

        let database = Database::open(&path)?;
        let stamped: i32 = database
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(stamped, super::SCHEMA_VERSION);
        Ok(())
    }

    #[test]
    fn node_search_stems_word_forms() -> Result<()> {
        let database = index_fixture("#+title: Derivative\n\nRate of change.\n")?;

        // Plural query stems to the same root as the singular title.
        let plural = database.search_nodes("derivatives", 20, None)?;
        assert!(
            plural.iter().any(|node| node.title == "Derivative"),
            "expected 'derivatives' to match the 'Derivative' note via stemming"
        );

        // And the reverse direction holds too.
        let database = index_fixture("#+title: Derivatives\n\nRates of change.\n")?;
        let singular = database.search_nodes("derivative", 20, None)?;
        assert!(
            singular.iter().any(|node| node.title == "Derivatives"),
            "expected 'derivative' to match the 'Derivatives' note via stemming"
        );

        Ok(())
    }

    #[test]
    fn node_search_folds_diacritics() -> Result<()> {
        let database = index_fixture("#+title: Gödel\n\nIncompleteness.\n")?;

        let folded = database.search_nodes("Godel", 20, None)?;
        assert!(
            folded.iter().any(|node| node.title == "Gödel"),
            "expected 'Godel' to match the 'Gödel' note via diacritic folding"
        );

        Ok(())
    }
}

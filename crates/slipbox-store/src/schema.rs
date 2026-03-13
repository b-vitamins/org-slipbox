use anyhow::Result;

use crate::Database;

const SCHEMA_VERSION: i32 = 12;

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
        self.connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;

             DROP TABLE IF EXISTS links;
             DROP TABLE IF EXISTS aliases;
             DROP TABLE IF EXISTS tags;
             DROP TABLE IF EXISTS refs;
             DROP TABLE IF EXISTS occurrence_document_fts;
             DROP TABLE IF EXISTS occurrence_documents;
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
               level INTEGER NOT NULL,
               line INTEGER NOT NULL,
               kind TEXT NOT NULL
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS node_fts USING fts5(
               title,
               outline_path,
               file_path,
               alias_text,
               ref_text,
               tag_text
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
               destination_explicit_id TEXT NOT NULL,
               line INTEGER NOT NULL,
               column INTEGER NOT NULL,
               preview TEXT NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_nodes_file_path
               ON nodes (file_path);

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

             CREATE INDEX IF NOT EXISTS idx_refs_ref
               ON refs (ref);

             CREATE INDEX IF NOT EXISTS idx_aliases_alias
               ON aliases (alias);

             CREATE INDEX IF NOT EXISTS idx_aliases_alias_nocase
               ON aliases (alias COLLATE NOCASE);

             CREATE INDEX IF NOT EXISTS idx_tags_tag
               ON tags (tag);

             CREATE INDEX IF NOT EXISTS idx_nodes_scheduled_for
               ON nodes (scheduled_for)
               WHERE scheduled_for IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_nodes_deadline_for
               ON nodes (deadline_for)
               WHERE deadline_for IS NOT NULL;

             PRAGMA user_version = 12;",
        )?;
        Ok(())
    }
}

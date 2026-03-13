use std::collections::HashSet;

use anyhow::{Context, Result};
use rusqlite::{Transaction, params};
use slipbox_core::{IndexStats, IndexedFile};

use crate::Database;

impl Database {
    pub fn sync_index(&mut self, files: &[IndexedFile]) -> Result<IndexStats> {
        let mut stats = IndexStats::default();
        let present_paths = files
            .iter()
            .map(|file| file.file_path.clone())
            .collect::<HashSet<_>>();

        for file in files {
            let file_stats = self.replace_file_index(file)?;
            stats.accumulate(&file_stats);
        }

        self.prune_missing_files(&present_paths)?;
        Ok(stats)
    }

    pub fn sync_file_index(&mut self, file: &IndexedFile) -> Result<IndexStats> {
        self.replace_file_index(file)
    }

    pub fn remove_file_index(&mut self, file_path: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        delete_file_rows(&transaction, file_path)?;
        transaction.commit()?;
        Ok(())
    }

    fn replace_file_index(&mut self, file: &IndexedFile) -> Result<IndexStats> {
        let transaction = self.connection.transaction()?;
        delete_file_rows(&transaction, &file.file_path)?;

        transaction.execute(
            "INSERT INTO files (path, title, mtime_ns)
             VALUES (?1, ?2, ?3)",
            params![file.file_path, file.title, file.mtime_ns],
        )?;

        for node in &file.nodes {
            transaction.execute(
                "INSERT INTO nodes (
                   node_key,
                   explicit_id,
                   file_path,
                   title,
                   outline_path,
                   aliases_json,
                   tags_json,
                   refs_json,
                   todo_keyword,
                   scheduled_for,
                   deadline_for,
                   closed_at,
                   level,
                   line,
                   kind
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    node.node_key,
                    node.explicit_id,
                    node.file_path,
                    node.title,
                    node.outline_path,
                    serde_json::to_string(&node.aliases)
                        .context("failed to serialize node aliases")?,
                    serde_json::to_string(&node.tags).context("failed to serialize node tags")?,
                    serde_json::to_string(&node.refs).context("failed to serialize node refs")?,
                    node.todo_keyword,
                    node.scheduled_for,
                    node.deadline_for,
                    node.closed_at,
                    node.level,
                    node.line,
                    node.kind.as_str(),
                ],
            )?;

            let row_id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO node_fts (rowid, title, outline_path, file_path, alias_text, ref_text, tag_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    row_id,
                    node.title,
                    node.outline_path,
                    node.file_path,
                    node.aliases.join(" "),
                    node.refs.join(" "),
                    node.tags.join(" ")
                ],
            )?;

            for reference in &node.refs {
                transaction.execute(
                    "INSERT INTO refs (node_key, ref)
                     VALUES (?1, ?2)",
                    params![node.node_key, reference],
                )?;
            }

            for alias in &node.aliases {
                transaction.execute(
                    "INSERT INTO aliases (node_key, alias)
                     VALUES (?1, ?2)",
                    params![node.node_key, alias],
                )?;
            }

            for tag in &node.tags {
                transaction.execute(
                    "INSERT INTO tags (node_key, tag)
                     VALUES (?1, ?2)",
                    params![node.node_key, tag],
                )?;
            }
        }

        for link in &file.links {
            transaction.execute(
                "INSERT INTO links (
                   source_node_key,
                   destination_explicit_id,
                   line,
                   column,
                   preview
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    link.source_node_key,
                    link.destination_explicit_id,
                    link.line,
                    link.column,
                    link.preview
                ],
            )?;
        }

        if let Some(occurrence_document) = &file.occurrence_document {
            transaction.execute(
                "INSERT INTO occurrence_documents (
                   file_path,
                   search_text,
                   line_rows_json
                 )
                 VALUES (?1, ?2, ?3)",
                params![
                    occurrence_document.file_path,
                    occurrence_document.search_text,
                    serde_json::to_string(&occurrence_document.line_rows)
                        .context("failed to serialize occurrence document line rows")?
                ],
            )?;
            let row_id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO occurrence_document_fts (rowid, search_text)
                 VALUES (?1, ?2)",
                params![row_id, occurrence_document.search_text],
            )?;
        }

        transaction.commit()?;

        Ok(IndexStats {
            files_indexed: 1,
            nodes_indexed: file.nodes.len() as u64,
            links_indexed: file.links.len() as u64,
        })
    }

    fn prune_missing_files(&mut self, present_paths: &HashSet<String>) -> Result<()> {
        let indexed_paths = {
            let mut statement = self.connection.prepare("SELECT path FROM files")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read indexed file list")?
        };

        for path in indexed_paths {
            if !present_paths.contains(&path) {
                let transaction = self.connection.transaction()?;
                delete_file_rows(&transaction, &path)?;
                transaction.commit()?;
            }
        }

        Ok(())
    }
}

fn delete_file_rows(transaction: &Transaction<'_>, file_path: &str) -> Result<()> {
    transaction.execute(
        "DELETE FROM aliases
          WHERE node_key IN (
                SELECT node_key
                  FROM nodes
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM tags
          WHERE node_key IN (
                SELECT node_key
                  FROM nodes
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM refs
          WHERE node_key IN (
                SELECT node_key
                  FROM nodes
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM links
          WHERE source_node_key IN (
                SELECT node_key
                  FROM nodes
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM occurrence_document_fts
          WHERE rowid IN (
                SELECT id
                  FROM occurrence_documents
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM occurrence_documents WHERE file_path = ?1",
        params![file_path],
    )?;
    transaction.execute(
        "DELETE FROM node_fts
          WHERE rowid IN (
                SELECT id
                  FROM nodes
                 WHERE file_path = ?1
          )",
        params![file_path],
    )?;
    transaction.execute("DELETE FROM nodes WHERE file_path = ?1", params![file_path])?;
    transaction.execute("DELETE FROM files WHERE path = ?1", params![file_path])?;
    Ok(())
}

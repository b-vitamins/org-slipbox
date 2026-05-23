use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use rusqlite::{Transaction, params};
use slipbox_core::{IndexStats, IndexedFile, IndexedNode, NodeKind};

use crate::Database;

impl Database {
    pub fn sync_index(&mut self, files: &[IndexedFile]) -> Result<IndexStats> {
        let present_paths = files
            .iter()
            .map(|file| file.file_path.clone())
            .collect::<HashSet<_>>();

        let stats = self.sync_file_indexes(files)?;
        self.prune_missing_files(&present_paths)?;
        Ok(stats)
    }

    pub fn sync_file_index(&mut self, file: &IndexedFile) -> Result<IndexStats> {
        self.sync_file_indexes(std::slice::from_ref(file))
    }

    pub fn sync_file_indexes(&mut self, files: &[IndexedFile]) -> Result<IndexStats> {
        let transaction = self.connection.transaction()?;

        for file in files {
            delete_file_rows(&transaction, &file.file_path)?;
        }

        let mut stats = IndexStats::default();
        for file in files {
            let file_stats = insert_file_rows(&transaction, file)?;
            stats.accumulate(&file_stats);
        }

        transaction.commit()?;
        Ok(stats)
    }

    pub fn remove_file_index(&mut self, file_path: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        delete_file_rows(&transaction, file_path)?;
        transaction.commit()?;
        Ok(())
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

fn insert_file_rows(transaction: &Transaction<'_>, file: &IndexedFile) -> Result<IndexStats> {
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
                serde_json::to_string(&node.aliases).context("failed to serialize node aliases")?,
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

    let source_note_keys = source_note_keys_by_anchor_key(&file.nodes);
    for link in &file.links {
        let source_note_key = source_note_keys
            .get(&link.source_node_key)
            .with_context(|| {
                format!(
                    "failed to resolve source note for indexed link anchor {}",
                    link.source_node_key
                )
            })?;
        transaction.execute(
            "INSERT INTO links (
                   source_node_key,
                   source_note_key,
                   source_file_path,
                   destination_explicit_id,
                   line,
                   column,
                   preview
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                link.source_node_key,
                source_note_key,
                file.file_path,
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

    Ok(IndexStats {
        files_indexed: 1,
        nodes_indexed: file.nodes.len() as u64,
        links_indexed: file.links.len() as u64,
    })
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
          WHERE source_file_path = ?1",
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

fn source_note_keys_by_anchor_key(nodes: &[IndexedNode]) -> HashMap<String, String> {
    let mut owners = HashMap::new();
    let mut ancestry: Vec<&IndexedNode> = Vec::new();
    let mut note_stack: Vec<&IndexedNode> = Vec::new();

    for node in nodes {
        while let Some(last) = ancestry.last().copied() {
            if matches!(last.kind, NodeKind::File) && !matches!(node.kind, NodeKind::File) {
                break;
            }
            if last.level < node.level {
                break;
            }
            ancestry.pop();
            if note_stack
                .last()
                .is_some_and(|candidate| candidate.node_key == last.node_key)
            {
                note_stack.pop();
            }
        }

        let owner_key = if indexed_node_is_note(node) {
            node.node_key.as_str()
        } else {
            note_stack
                .last()
                .map(|candidate| candidate.node_key.as_str())
                .unwrap_or(node.node_key.as_str())
        };
        owners.insert(node.node_key.clone(), owner_key.to_owned());

        ancestry.push(node);
        if indexed_node_is_note(node) {
            note_stack.push(node);
        }
    }

    owners
}

fn indexed_node_is_note(node: &IndexedNode) -> bool {
    matches!(node.kind, NodeKind::File) || node.explicit_id.is_some()
}

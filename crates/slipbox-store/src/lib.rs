use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use slipbox_core::{IndexStats, IndexedFile, NodeKind, NodeRecord};

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create database directory {}", parent.display())
            })?;
        }

        let connection = Connection::open(path)
            .with_context(|| format!("failed to open database {}", path.display()))?;
        let database = Self { connection };
        database.initialize()?;
        Ok(database)
    }

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

    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<NodeRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if let Some(fts_query) = build_fts_query(query) {
            let mut statement = self.connection.prepare(
                "SELECT n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.level,
                        n.line,
                        n.kind
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                  ORDER BY bm25(node_fts, 1.0, 0.4, 0.2), n.file_path, n.line
                  LIMIT ?2",
            )?;
            let rows = statement.query_map(params![fts_query, limit], row_to_node)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read search results")
        } else {
            let mut statement = self.connection.prepare(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        level,
                        line,
                        kind
                   FROM nodes
                  ORDER BY file_path, line
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_node)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read node listing")
        }
    }

    pub fn backlinks(&self, node_key: &str, limit: usize) -> Result<Vec<NodeRecord>> {
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
            .context("failed to resolve node for backlink lookup")?
            .flatten();

        let Some(explicit_id) = explicit_id else {
            return Ok(Vec::new());
        };

        let mut statement = self.connection.prepare(
            "SELECT DISTINCT n.node_key,
                             n.explicit_id,
                             n.file_path,
                             n.title,
                             n.outline_path,
                             n.level,
                             n.line,
                             n.kind
               FROM links AS l
               JOIN nodes AS n ON n.node_key = l.source_node_key
              WHERE l.destination_explicit_id = ?1
              ORDER BY n.file_path, n.line
              LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![explicit_id, limit.clamp(1, 1_000) as i64],
            row_to_node,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read backlinks")
    }

    pub fn node_by_key(&self, node_key: &str) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
                        explicit_id,
                        file_path,
                        title,
                        outline_path,
                        level,
                        line,
                        kind
                   FROM nodes
                  WHERE node_key = ?1",
                params![node_key],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node by key")
    }

    pub fn remove_file_index(&mut self, file_path: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        delete_file_rows(&transaction, file_path)?;
        transaction.commit()?;
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;

             CREATE TABLE IF NOT EXISTS files (
               path TEXT PRIMARY KEY,
               mtime_ns INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS nodes (
               id INTEGER PRIMARY KEY,
               node_key TEXT NOT NULL UNIQUE,
               explicit_id TEXT UNIQUE,
               file_path TEXT NOT NULL,
               title TEXT NOT NULL,
               outline_path TEXT NOT NULL,
               level INTEGER NOT NULL,
               line INTEGER NOT NULL,
               kind TEXT NOT NULL
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS node_fts USING fts5(
               title,
               outline_path,
               file_path
             );

             CREATE TABLE IF NOT EXISTS links (
               source_node_key TEXT NOT NULL,
               destination_explicit_id TEXT NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_nodes_file_path
               ON nodes (file_path);

             CREATE INDEX IF NOT EXISTS idx_nodes_explicit_id
               ON nodes (explicit_id)
               WHERE explicit_id IS NOT NULL;

             CREATE INDEX IF NOT EXISTS idx_links_source_node_key
               ON links (source_node_key);

             CREATE INDEX IF NOT EXISTS idx_links_destination_explicit_id
               ON links (destination_explicit_id);",
        )?;
        Ok(())
    }

    fn replace_file_index(&mut self, file: &IndexedFile) -> Result<IndexStats> {
        let transaction = self.connection.transaction()?;
        delete_file_rows(&transaction, &file.file_path)?;

        transaction.execute(
            "INSERT INTO files (path, mtime_ns)
             VALUES (?1, ?2)",
            params![file.file_path, file.mtime_ns],
        )?;

        for node in &file.nodes {
            transaction.execute(
                "INSERT INTO nodes (
                   node_key,
                   explicit_id,
                   file_path,
                   title,
                   outline_path,
                   level,
                   line,
                   kind
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    node.node_key,
                    node.explicit_id,
                    node.file_path,
                    node.title,
                    node.outline_path,
                    node.level,
                    node.line,
                    node.kind.as_str(),
                ],
            )?;

            let row_id = transaction.last_insert_rowid();
            transaction.execute(
                "INSERT INTO node_fts (rowid, title, outline_path, file_path)
                 VALUES (?1, ?2, ?3, ?4)",
                params![row_id, node.title, node.outline_path, node.file_path],
            )?;
        }

        for link in &file.links {
            transaction.execute(
                "INSERT INTO links (source_node_key, destination_explicit_id)
                 VALUES (?1, ?2)",
                params![link.source_node_key, link.destination_explicit_id],
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

fn delete_file_rows(transaction: &rusqlite::Transaction<'_>, file_path: &str) -> Result<()> {
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

fn row_to_node(row: &rusqlite::Row<'_>) -> rusqlite::Result<NodeRecord> {
    let kind_text: String = row.get(7)?;
    Ok(NodeRecord {
        node_key: row.get(0)?,
        explicit_id: row.get(1)?,
        file_path: row.get(2)?,
        title: row.get(3)?,
        outline_path: row.get(4)?,
        level: row.get(5)?,
        line: row.get(6)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
    })
}

fn build_fts_query(input: &str) -> Option<String> {
    let tokens = input
        .split_whitespace()
        .filter_map(|token| {
            let cleaned = token
                .chars()
                .filter(|character| character.is_alphanumeric() || matches!(character, '_' | '-'))
                .collect::<String>();
            if cleaned.is_empty() {
                None
            } else {
                Some(format!("{cleaned}*"))
            }
        })
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

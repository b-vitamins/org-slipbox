use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use slipbox_core::{
    BacklinkRecord, IndexStats, IndexedFile, NodeKind, NodeRecord, RefRecord, normalize_reference,
};

const SCHEMA_VERSION: i32 = 7;

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
        database.migrate()?;
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

    pub fn sync_file_index(&mut self, file: &IndexedFile) -> Result<IndexStats> {
        self.replace_file_index(file)
    }

    pub fn stats(&self) -> Result<IndexStats> {
        Ok(IndexStats {
            files_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM files", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed files")?,
            nodes_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed nodes")?,
            links_indexed: self
                .connection
                .query_row("SELECT COUNT(*) FROM links", [], |row| row.get::<_, u64>(0))
                .context("failed to count indexed links")?,
        })
    }

    pub fn indexed_files(&self) -> Result<Vec<String>> {
        let mut statement = self
            .connection
            .prepare("SELECT path FROM files ORDER BY path")?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read indexed file list")
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
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind
                   FROM node_fts
                   JOIN nodes AS n ON n.id = node_fts.rowid
                  WHERE node_fts MATCH ?1
                  ORDER BY bm25(node_fts, 1.0, 0.3, 0.2, 0.7, 0.8, 0.4), n.file_path, n.line
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
                   FROM nodes
                  ORDER BY file_path, line
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_node)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read node listing")
        }
    }

    pub fn random_node(&self) -> Result<Option<NodeRecord>> {
        let Some((min_id, max_id)) = self
            .connection
            .query_row(
                "SELECT MIN(id), MAX(id)
                   FROM nodes",
                [],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()
            .context("failed to determine node ID bounds")?
        else {
            return Ok(None);
        };

        let (Some(min_id), Some(max_id)) = (min_id, max_id) else {
            return Ok(None);
        };

        self.connection
            .query_row(
                "SELECT node_key,
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
                   FROM nodes
                  WHERE id >= ((ABS(random()) % (?2 - ?1 + 1)) + ?1)
                  ORDER BY id
                  LIMIT 1",
                params![min_id, max_id],
                row_to_node,
            )
            .optional()
            .context("failed to fetch random node")
    }

    pub fn search_tags(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let limit = limit.clamp(1, 1_000) as i64;
        if query.trim().is_empty() {
            let mut statement = self.connection.prepare(
                "SELECT DISTINCT tag
                   FROM tags
                  ORDER BY tag
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read tags")
        } else {
            let pattern = format!("{}%", escape_like_pattern(query.trim()));
            let mut statement = self.connection.prepare(
                "SELECT DISTINCT tag
                   FROM tags
                  WHERE tag LIKE ?1 ESCAPE '\\' COLLATE NOCASE
                  ORDER BY tag
                  LIMIT ?2",
            )?;
            let rows =
                statement.query_map(params![pattern, limit], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to search tags")
        }
    }

    pub fn node_from_id(&self, explicit_id: &str) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
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
                   FROM nodes
                  WHERE explicit_id = ?1
                  LIMIT 1",
                params![explicit_id],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node from ID")
    }

    pub fn node_from_title_or_alias(
        &self,
        title_or_alias: &str,
        nocase: bool,
    ) -> Result<Vec<NodeRecord>> {
        let sql = if nocase {
            "SELECT DISTINCT n.node_key,
                             n.explicit_id,
                             n.file_path,
                             n.title,
                             n.outline_path,
                             n.aliases_json,
                             n.tags_json,
                             n.refs_json,
                             n.todo_keyword,
                             n.scheduled_for,
                             n.deadline_for,
                             n.closed_at,
                             n.level,
                             n.line,
                             n.kind
               FROM nodes AS n
               LEFT JOIN aliases AS a ON a.node_key = n.node_key
              WHERE n.title = ?1 COLLATE NOCASE
                 OR a.alias = ?1 COLLATE NOCASE
              ORDER BY n.file_path, n.line
              LIMIT 2"
        } else {
            "SELECT DISTINCT n.node_key,
                             n.explicit_id,
                             n.file_path,
                             n.title,
                             n.outline_path,
                             n.aliases_json,
                             n.tags_json,
                             n.refs_json,
                             n.todo_keyword,
                             n.scheduled_for,
                             n.deadline_for,
                             n.closed_at,
                             n.level,
                             n.line,
                             n.kind
               FROM nodes AS n
               LEFT JOIN aliases AS a ON a.node_key = n.node_key
              WHERE n.title = ?1
                 OR a.alias = ?1
              ORDER BY n.file_path, n.line
              LIMIT 2"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map(params![title_or_alias], row_to_node)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to fetch node from title or alias")
    }

    pub fn node_at_point(&self, file_path: &str, line: u32) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
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
                   FROM nodes
                  WHERE file_path = ?1
                    AND line <= ?2
                  ORDER BY line DESC, level DESC
                  LIMIT 1",
                params![file_path, line],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node at point")
    }

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
            .context("failed to resolve node for backlink lookup")?
            .flatten();

        let Some(explicit_id) = explicit_id else {
            return Ok(Vec::new());
        };

        let mut statement = if unique {
            self.connection.prepare(
                "SELECT n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind,
                        l.line,
                        l.column,
                        l.preview
                   FROM (
                         SELECT source_node_key,
                                line,
                                column,
                                preview,
                                ROW_NUMBER() OVER (
                                  PARTITION BY source_node_key
                                  ORDER BY line, column
                                ) AS occurrence_rank
                           FROM links
                          WHERE destination_explicit_id = ?1
                        ) AS l
                   JOIN nodes AS n ON n.node_key = l.source_node_key
                  WHERE l.occurrence_rank = 1
                  ORDER BY n.file_path, l.line, l.column
                  LIMIT ?2",
            )?
        } else {
            self.connection.prepare(
                "SELECT n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind,
                        l.line,
                        l.column,
                        l.preview
                   FROM links AS l
                   JOIN nodes AS n ON n.node_key = l.source_node_key
                  WHERE l.destination_explicit_id = ?1
                  ORDER BY n.file_path, l.line, l.column
                  LIMIT ?2",
            )?
        };
        let rows = statement.query_map(
            params![explicit_id, limit.clamp(1, 1_000) as i64],
            row_to_backlink,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read backlinks")
    }

    pub fn search_refs(&self, query: &str, limit: usize) -> Result<Vec<RefRecord>> {
        let limit = limit.clamp(1, 200) as i64;
        if query.trim().is_empty() {
            let mut statement = self.connection.prepare(
                "SELECT r.ref,
                        n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], row_to_ref)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to read refs")
        } else {
            let query = query.trim();
            let normalized = normalize_reference(query)
                .into_iter()
                .next()
                .unwrap_or_else(|| query.to_owned());
            let bare = query.trim_start_matches('@');
            let mut statement = self.connection.prepare(
                "SELECT r.ref,
                        n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE r.ref LIKE ?1
                     OR r.ref LIKE ?2
                     OR r.ref LIKE ?3
                  ORDER BY r.ref, n.file_path, n.line
                  LIMIT ?4",
            )?;
            let rows = statement.query_map(
                params![
                    format!("{query}%"),
                    format!("{normalized}%"),
                    format!("@{bare}%"),
                    limit
                ],
                row_to_ref,
            )?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("failed to search refs")
        }
    }

    pub fn agenda_nodes(&self, start: &str, end: &str, limit: usize) -> Result<Vec<NodeRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT node_key,
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
               FROM nodes
              WHERE (scheduled_for IS NOT NULL AND scheduled_for >= ?1 AND scheduled_for <= ?2)
                 OR (deadline_for IS NOT NULL AND deadline_for >= ?1 AND deadline_for <= ?2)
              ORDER BY COALESCE(scheduled_for, deadline_for), file_path, line
              LIMIT ?3",
        )?;
        let rows =
            statement.query_map(params![start, end, limit.clamp(1, 500) as i64], row_to_node)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .context("failed to read agenda nodes")
    }

    pub fn node_from_ref(&self, reference: &str) -> Result<Option<NodeRecord>> {
        let normalized = normalize_reference(reference);
        let Some(reference) = normalized.first() else {
            return Ok(None);
        };

        self.connection
            .query_row(
                "SELECT n.node_key,
                        n.explicit_id,
                        n.file_path,
                        n.title,
                        n.outline_path,
                        n.aliases_json,
                        n.tags_json,
                        n.refs_json,
                        n.todo_keyword,
                        n.scheduled_for,
                        n.deadline_for,
                        n.closed_at,
                        n.level,
                        n.line,
                        n.kind
                   FROM refs AS r
                   JOIN nodes AS n ON n.node_key = r.node_key
                  WHERE r.ref = ?1
                  ORDER BY n.file_path, n.line
                  LIMIT 1",
                params![reference],
                row_to_node,
            )
            .optional()
            .context("failed to fetch node from ref")
    }

    pub fn node_by_key(&self, node_key: &str) -> Result<Option<NodeRecord>> {
        self.connection
            .query_row(
                "SELECT node_key,
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

    fn migrate(&self) -> Result<()> {
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
             DROP TABLE IF EXISTS node_fts;
             DROP TABLE IF EXISTS nodes;
             DROP TABLE IF EXISTS files;

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

             PRAGMA user_version = 7;",
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
    let kind_text: String = row.get(14)?;
    Ok(NodeRecord {
        node_key: row.get(0)?,
        explicit_id: row.get(1)?,
        file_path: row.get(2)?,
        title: row.get(3)?,
        outline_path: row.get(4)?,
        aliases: parse_string_list(row.get::<_, String>(5)?),
        tags: parse_string_list(row.get::<_, String>(6)?),
        refs: parse_string_list(row.get::<_, String>(7)?),
        todo_keyword: row.get(8)?,
        scheduled_for: row.get(9)?,
        deadline_for: row.get(10)?,
        closed_at: row.get(11)?,
        level: row.get(12)?,
        line: row.get(13)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
    })
}

fn row_to_ref(row: &rusqlite::Row<'_>) -> rusqlite::Result<RefRecord> {
    Ok(RefRecord {
        reference: row.get(0)?,
        node: row_to_node_with_offset(row, 1)?,
    })
}

fn row_to_backlink(row: &rusqlite::Row<'_>) -> rusqlite::Result<BacklinkRecord> {
    Ok(BacklinkRecord {
        source_node: row_to_node_with_offset(row, 0)?,
        row: row.get(15)?,
        col: row.get(16)?,
        preview: row.get(17)?,
    })
}

fn row_to_node_with_offset(row: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<NodeRecord> {
    let kind_text: String = row.get(offset + 14)?;
    Ok(NodeRecord {
        node_key: row.get(offset)?,
        explicit_id: row.get(offset + 1)?,
        file_path: row.get(offset + 2)?,
        title: row.get(offset + 3)?,
        outline_path: row.get(offset + 4)?,
        aliases: parse_string_list(row.get::<_, String>(offset + 5)?),
        tags: parse_string_list(row.get::<_, String>(offset + 6)?),
        refs: parse_string_list(row.get::<_, String>(offset + 7)?),
        todo_keyword: row.get(offset + 8)?,
        scheduled_for: row.get(offset + 9)?,
        deadline_for: row.get(offset + 10)?,
        closed_at: row.get(offset + 11)?,
        level: row.get(offset + 12)?,
        line: row.get(offset + 13)?,
        kind: kind_text.parse().unwrap_or(NodeKind::Heading),
    })
}

fn parse_string_list(value: String) -> Vec<String> {
    match serde_json::from_str::<Value>(&value) {
        Ok(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::String(string) if !string.is_empty() => Some(string),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
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

fn escape_like_pattern(input: &str) -> String {
    input
        .chars()
        .flat_map(|character| match character {
            '\\' => ['\\', '\\'].into_iter().collect::<Vec<_>>(),
            '%' => ['\\', '%'].into_iter().collect::<Vec<_>>(),
            '_' => ['\\', '_'].into_iter().collect::<Vec<_>>(),
            _ => [character].into_iter().collect::<Vec<_>>(),
        })
        .collect()
}

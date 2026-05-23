use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use rusqlite::{Transaction, params, params_from_iter};
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
        let changed_paths = files
            .iter()
            .map(|file| file.file_path.clone())
            .collect::<Vec<_>>();
        let rebuild_counts = should_rebuild_relation_counts(files);
        let old_destination_ids = if rebuild_counts {
            HashSet::new()
        } else {
            indexed_explicit_ids_for_paths(&transaction, &changed_paths)?
        };
        let new_destination_ids = if rebuild_counts {
            HashSet::new()
        } else {
            scanned_explicit_ids(files)
        };
        let removed_destination_ids = difference(&old_destination_ids, &new_destination_ids);
        let added_destination_ids = difference(&new_destination_ids, &old_destination_ids);

        if !rebuild_counts {
            apply_backlink_delta_for_source_paths(&transaction, &changed_paths, -1)?;
            apply_external_forward_delta_for_destination_ids(
                &transaction,
                &removed_destination_ids,
                &changed_paths,
                -1,
            )?;
        }

        for file in files {
            delete_file_rows(&transaction, &file.file_path)?;
        }

        let mut stats = IndexStats::default();
        for file in files {
            let file_stats = insert_file_rows(&transaction, file)?;
            stats.accumulate(&file_stats);
        }
        if rebuild_counts {
            rebuild_relation_counts(&transaction)?;
        } else {
            apply_backlink_delta_for_source_paths(&transaction, &changed_paths, 1)?;
            apply_forward_delta_for_source_paths(&transaction, &changed_paths, 1)?;
            apply_external_backlink_delta_for_destination_ids(
                &transaction,
                &new_destination_ids,
                &changed_paths,
                1,
            )?;
            apply_external_forward_delta_for_destination_ids(
                &transaction,
                &added_destination_ids,
                &changed_paths,
                1,
            )?;
        }

        transaction.commit()?;
        Ok(stats)
    }

    pub fn remove_file_index(&mut self, file_path: &str) -> Result<()> {
        let transaction = self.connection.transaction()?;
        let changed_paths = vec![file_path.to_owned()];
        let old_destination_ids = indexed_explicit_ids_for_paths(&transaction, &changed_paths)?;
        apply_backlink_delta_for_source_paths(&transaction, &changed_paths, -1)?;
        apply_external_forward_delta_for_destination_ids(
            &transaction,
            &old_destination_ids,
            &changed_paths,
            -1,
        )?;
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
                let changed_paths = vec![path.clone()];
                let old_destination_ids =
                    indexed_explicit_ids_for_paths(&transaction, &changed_paths)?;
                apply_backlink_delta_for_source_paths(&transaction, &changed_paths, -1)?;
                apply_external_forward_delta_for_destination_ids(
                    &transaction,
                    &old_destination_ids,
                    &changed_paths,
                    -1,
                )?;
                delete_file_rows(&transaction, &path)?;
                transaction.commit()?;
            }
        }

        Ok(())
    }
}

const RELATION_COUNT_REBUILD_FILE_THRESHOLD: usize = 32;
const RELATION_COUNT_REBUILD_ROW_THRESHOLD: usize = 4_096;
const SQLITE_PARAM_CHUNK: usize = 900;

fn should_rebuild_relation_counts(files: &[IndexedFile]) -> bool {
    files.len() > RELATION_COUNT_REBUILD_FILE_THRESHOLD
        || files
            .iter()
            .map(|file| file.nodes.len() + file.links.len())
            .sum::<usize>()
            > RELATION_COUNT_REBUILD_ROW_THRESHOLD
}

fn difference(left: &HashSet<String>, right: &HashSet<String>) -> HashSet<String> {
    left.difference(right).cloned().collect()
}

fn rebuild_relation_counts(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(
        "DROP TABLE IF EXISTS temp_relation_backlink_counts;
         DROP TABLE IF EXISTS temp_relation_forward_link_counts;

         CREATE TEMP TABLE temp_relation_backlink_counts (
           explicit_id TEXT PRIMARY KEY,
           backlink_count INTEGER NOT NULL
         ) WITHOUT ROWID;

         INSERT INTO temp_relation_backlink_counts (explicit_id, backlink_count)
         SELECT destination_explicit_id, COUNT(*)
           FROM links
          GROUP BY destination_explicit_id;

         UPDATE nodes
            SET backlink_count = COALESCE((
                  SELECT counts.backlink_count
                    FROM temp_relation_backlink_counts AS counts
                   WHERE counts.explicit_id = nodes.explicit_id
                ), 0)
          WHERE kind = 'file' OR explicit_id IS NOT NULL;

         CREATE TEMP TABLE temp_relation_forward_link_counts (
           node_key TEXT PRIMARY KEY,
           forward_link_count INTEGER NOT NULL
         ) WITHOUT ROWID;

         INSERT INTO temp_relation_forward_link_counts (node_key, forward_link_count)
         SELECT outgoing.source_note_key, COUNT(*)
           FROM links AS outgoing
           JOIN nodes AS dest ON dest.explicit_id = outgoing.destination_explicit_id
          GROUP BY outgoing.source_note_key;

         UPDATE nodes
            SET forward_link_count = COALESCE((
                  SELECT counts.forward_link_count
                    FROM temp_relation_forward_link_counts AS counts
                   WHERE counts.node_key = nodes.node_key
                ), 0)
          WHERE kind = 'file' OR explicit_id IS NOT NULL;

         DROP TABLE IF EXISTS temp_relation_backlink_counts;
         DROP TABLE IF EXISTS temp_relation_forward_link_counts;",
    )?;
    Ok(())
}

fn indexed_explicit_ids_for_paths(
    transaction: &Transaction<'_>,
    file_paths: &[String],
) -> Result<HashSet<String>> {
    let mut explicit_ids = HashSet::new();
    if file_paths.is_empty() {
        return Ok(explicit_ids);
    }
    let sql = format!(
        "SELECT explicit_id
           FROM nodes
          WHERE file_path IN ({})
            AND explicit_id IS NOT NULL",
        placeholders(file_paths.len())
    );
    let mut statement = transaction.prepare(&sql)?;
    let rows = statement.query_map(
        params_from_iter(file_paths.iter().map(String::as_str)),
        |row| row.get::<_, String>(0),
    )?;
    for row in rows {
        explicit_ids.insert(row?);
    }
    Ok(explicit_ids)
}

fn scanned_explicit_ids(files: &[IndexedFile]) -> HashSet<String> {
    files
        .iter()
        .flat_map(|file| file.nodes.iter())
        .filter_map(|node| node.explicit_id.clone())
        .collect()
}

fn apply_backlink_delta_for_source_paths(
    transaction: &Transaction<'_>,
    file_paths: &[String],
    sign: i64,
) -> Result<()> {
    if file_paths.is_empty() {
        return Ok(());
    }
    let multiplier = delta_multiplier(sign);
    transaction.execute_batch(
        "DROP TABLE IF EXISTS temp_relation_backlink_delta;
         CREATE TEMP TABLE temp_relation_backlink_delta (
           explicit_id TEXT PRIMARY KEY,
           delta INTEGER NOT NULL
         ) WITHOUT ROWID;",
    )?;
    let sql = format!(
        "INSERT INTO temp_relation_backlink_delta (explicit_id, delta)
         SELECT destination_explicit_id, COUNT(*) * {multiplier}
           FROM links
          WHERE source_file_path IN ({})
          GROUP BY destination_explicit_id",
        placeholders(file_paths.len())
    );
    transaction.execute(
        &sql,
        params_from_iter(file_paths.iter().map(String::as_str)),
    )?;
    transaction.execute(
        "UPDATE nodes
            SET backlink_count = backlink_count + (
                  SELECT delta
                    FROM temp_relation_backlink_delta AS delta
                   WHERE delta.explicit_id = nodes.explicit_id
                )
          WHERE explicit_id IN (
                SELECT explicit_id
                  FROM temp_relation_backlink_delta
          )",
        [],
    )?;
    transaction.execute_batch("DROP TABLE IF EXISTS temp_relation_backlink_delta;")?;
    Ok(())
}

fn apply_forward_delta_for_source_paths(
    transaction: &Transaction<'_>,
    file_paths: &[String],
    sign: i64,
) -> Result<()> {
    if file_paths.is_empty() {
        return Ok(());
    }
    let multiplier = delta_multiplier(sign);
    transaction.execute_batch(
        "DROP TABLE IF EXISTS temp_relation_forward_delta;
         CREATE TEMP TABLE temp_relation_forward_delta (
           node_key TEXT PRIMARY KEY,
           delta INTEGER NOT NULL
         ) WITHOUT ROWID;",
    )?;
    let sql = format!(
        "INSERT INTO temp_relation_forward_delta (node_key, delta)
         SELECT outgoing.source_note_key, COUNT(*) * {multiplier}
           FROM links AS outgoing
           JOIN nodes AS dest ON dest.explicit_id = outgoing.destination_explicit_id
          WHERE outgoing.source_file_path IN ({})
          GROUP BY outgoing.source_note_key",
        placeholders(file_paths.len())
    );
    transaction.execute(
        &sql,
        params_from_iter(file_paths.iter().map(String::as_str)),
    )?;
    transaction.execute(
        "UPDATE nodes
            SET forward_link_count = forward_link_count + (
                  SELECT delta
                    FROM temp_relation_forward_delta AS delta
                   WHERE delta.node_key = nodes.node_key
                )
          WHERE node_key IN (
                SELECT node_key
                  FROM temp_relation_forward_delta
          )",
        [],
    )?;
    transaction.execute_batch("DROP TABLE IF EXISTS temp_relation_forward_delta;")?;
    Ok(())
}

fn apply_external_backlink_delta_for_destination_ids(
    transaction: &Transaction<'_>,
    explicit_ids: &HashSet<String>,
    excluded_source_paths: &[String],
    sign: i64,
) -> Result<()> {
    if explicit_ids.is_empty() {
        return Ok(());
    }
    let multiplier = delta_multiplier(sign);
    for chunk in sorted_chunks(explicit_ids) {
        transaction.execute_batch(
            "DROP TABLE IF EXISTS temp_relation_backlink_delta;
             CREATE TEMP TABLE temp_relation_backlink_delta (
               explicit_id TEXT PRIMARY KEY,
               delta INTEGER NOT NULL
             ) WITHOUT ROWID;",
        )?;
        let sql = format!(
            "INSERT INTO temp_relation_backlink_delta (explicit_id, delta)
             SELECT destination_explicit_id, COUNT(*) * {multiplier}
               FROM links
              WHERE destination_explicit_id IN ({})
                AND source_file_path NOT IN ({})
              GROUP BY destination_explicit_id",
            placeholders(chunk.len()),
            placeholders(excluded_source_paths.len())
        );
        let params = chunk
            .iter()
            .copied()
            .chain(excluded_source_paths.iter().map(String::as_str));
        transaction.execute(&sql, params_from_iter(params))?;
        transaction.execute(
            "UPDATE nodes
                SET backlink_count = backlink_count + (
                      SELECT delta
                        FROM temp_relation_backlink_delta AS delta
                       WHERE delta.explicit_id = nodes.explicit_id
                    )
              WHERE explicit_id IN (
                    SELECT explicit_id
                      FROM temp_relation_backlink_delta
              )",
            [],
        )?;
        transaction.execute_batch("DROP TABLE IF EXISTS temp_relation_backlink_delta;")?;
    }
    Ok(())
}

fn apply_external_forward_delta_for_destination_ids(
    transaction: &Transaction<'_>,
    explicit_ids: &HashSet<String>,
    excluded_source_paths: &[String],
    sign: i64,
) -> Result<()> {
    if explicit_ids.is_empty() {
        return Ok(());
    }
    let multiplier = delta_multiplier(sign);
    for chunk in sorted_chunks(explicit_ids) {
        transaction.execute_batch(
            "DROP TABLE IF EXISTS temp_relation_forward_delta;
             CREATE TEMP TABLE temp_relation_forward_delta (
               node_key TEXT PRIMARY KEY,
               delta INTEGER NOT NULL
             ) WITHOUT ROWID;",
        )?;
        let sql = format!(
            "INSERT INTO temp_relation_forward_delta (node_key, delta)
             SELECT source_note_key, COUNT(*) * {multiplier}
               FROM links
              WHERE destination_explicit_id IN ({})
                AND source_file_path NOT IN ({})
              GROUP BY source_note_key",
            placeholders(chunk.len()),
            placeholders(excluded_source_paths.len())
        );
        let params = chunk
            .iter()
            .copied()
            .chain(excluded_source_paths.iter().map(String::as_str));
        transaction.execute(&sql, params_from_iter(params))?;
        transaction.execute(
            "UPDATE nodes
                SET forward_link_count = forward_link_count + (
                      SELECT delta
                        FROM temp_relation_forward_delta AS delta
                       WHERE delta.node_key = nodes.node_key
                    )
              WHERE node_key IN (
                    SELECT node_key
                      FROM temp_relation_forward_delta
              )",
            [],
        )?;
        transaction.execute_batch("DROP TABLE IF EXISTS temp_relation_forward_delta;")?;
    }
    Ok(())
}

fn delta_multiplier(sign: i64) -> &'static str {
    if sign < 0 { "-1" } else { "1" }
}

fn sorted_chunks(values: &HashSet<String>) -> Vec<Vec<&str>> {
    let mut sorted = values.iter().map(String::as_str).collect::<Vec<_>>();
    sorted.sort_unstable();
    sorted
        .chunks(SQLITE_PARAM_CHUNK)
        .map(|chunk| chunk.to_vec())
        .collect()
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
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

mod admin;
mod agenda;
mod artifacts;
mod audits;
mod backlinks;
mod comparison;
mod exploration;
mod files;
mod forward_links;
mod graph;
mod json_store;
mod links;
mod nodes;
mod occurrences;
mod packs;
mod refs;
mod reviews;
mod schema;
mod sync;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

pub use nodes::{GlossaryPage, GlossaryPosition, note_owners_by_anchor_key};

pub struct Database {
    connection: Connection,
    artifact_store: artifacts::ExplorationArtifactStore,
    pack_store: packs::WorkbenchPackStore,
    review_store: reviews::ReviewRunStore,
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
        configure_connection(&connection)?;
        let artifact_store = artifacts::ExplorationArtifactStore::for_database_path(path);
        artifact_store.migrate()?;
        let pack_store = packs::WorkbenchPackStore::for_database_path(path);
        pack_store.migrate()?;
        let review_store = reviews::ReviewRunStore::for_database_path(path);
        review_store.migrate()?;
        let database = Self {
            connection,
            artifact_store,
            pack_store,
            review_store,
        };
        database.migrate()?;
        Ok(database)
    }
}

fn configure_connection(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::Database;

    #[test]
    fn opens_existing_databases_with_write_pragmas() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let db_path = workspace.path().join("index.sqlite3");
        {
            let _database = Database::open(&db_path)?;
        }

        let database = Database::open(&db_path)?;
        let journal_mode: String =
            database
                .connection
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        let foreign_keys: i64 =
            database
                .connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        let synchronous: i64 = database
            .connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))?;

        assert_eq!(journal_mode, "wal");
        assert_eq!(foreign_keys, 1);
        assert_eq!(synchronous, 1);
        Ok(())
    }
}

#[cfg(test)]
mod test_support {
    use std::fs;
    use std::path::PathBuf;

    use anyhow::{Context, Result};
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
    use tempfile::TempDir;

    use super::Database;

    pub(crate) fn indexed_database(files: &[(&str, &str)]) -> Result<(TempDir, Database, PathBuf)> {
        let workspace = tempfile::tempdir().context("workspace should be created")?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root).context("notes root should be created")?;
        for (name, contents) in files {
            fs::write(root.join(name), contents)
                .with_context(|| format!("fixture {} should be written", name))?;
        }

        let mut database = Database::open(&workspace.path().join("index.sqlite3"))?;
        let files =
            scan_root_with_policy(&root, &DiscoveryPolicy::default()).context("fixture scan")?;
        database.sync_index(&files).context("fixture index sync")?;
        Ok((workspace, database, root))
    }
}

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, MAIN_DB, OpenFlags};

use slipbox_core::IndexStats;

use crate::Database;
use crate::schema::INDEX_SCHEMA_VERSION;

impl Database {
    /// Copy a current index through SQLite's online-backup API.
    ///
    /// The source may have concurrent readers and a WAL file. An incompatible
    /// source is refused before the destination is created so callers must
    /// schedule a full rebuild rather than accidentally treating an empty
    /// migrated database as an incremental base.
    pub fn clone_current_index(source: &Path, destination: &Path) -> Result<()> {
        if destination.exists() {
            bail!("index clone destination already exists");
        }
        let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open incremental index base")?;
        require_current_schema(&source)?;
        verify_connection(&source)?;

        if let Err(error) = source.backup(MAIN_DB, destination, None) {
            let _ = fs::remove_file(destination);
            return Err(error).context("failed to clone incremental index base");
        }

        let cloned = Connection::open_with_flags(destination, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open cloned incremental index")?;
        if let Err(error) =
            require_current_schema(&cloned).and_then(|()| verify_connection(&cloned))
        {
            drop(cloned);
            let _ = fs::remove_file(destination);
            return Err(error).context("cloned incremental index is not usable");
        }
        Ok(())
    }

    /// Verify that this handle is a complete index at the current schema.
    pub fn verify_current_index(&self) -> Result<()> {
        require_current_schema(&self.connection)?;
        verify_connection(&self.connection)
    }

    /// Verify an immutable index without enabling migrations or write pragmas.
    pub fn verify_current_index_path(path: &Path) -> Result<()> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open immutable index")?;
        require_current_schema(&connection)?;
        verify_connection(&connection)
    }

    /// Read an immutable index's schema stamp without running migrations.
    pub fn index_schema_version(path: &Path) -> Result<i32> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open immutable index")?;
        connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .context("failed to read immutable index schema version")
    }

    /// Read immutable index counts without enabling migrations or write pragmas.
    pub fn current_index_stats(path: &Path) -> Result<IndexStats> {
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .context("failed to open immutable index")?;
        require_current_schema(&connection)?;
        verify_connection(&connection)?;
        connection_stats(&connection)
    }

    /// Checkpoint derived writes before an immutable index is handed off.
    pub fn finish_index_writes(&self) -> Result<()> {
        let busy: i64 = self
            .connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
            .context("failed to checkpoint completed index")?;
        if busy != 0 {
            bail!("completed index remained busy during checkpoint");
        }
        Ok(())
    }
}

fn require_current_schema(connection: &Connection) -> Result<()> {
    let version: i32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .context("failed to read index schema version")?;
    if version != INDEX_SCHEMA_VERSION {
        bail!(
            "incremental index schema {version} does not match current schema {INDEX_SCHEMA_VERSION}"
        );
    }
    Ok(())
}

fn verify_connection(connection: &Connection) -> Result<()> {
    let result: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .context("failed to verify index")?;
    if result != "ok" {
        bail!("index verification failed");
    }
    Ok(())
}

fn connection_stats(connection: &Connection) -> Result<IndexStats> {
    Ok(IndexStats {
        files_indexed: connection
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .context("failed to count immutable index files")?,
        nodes_indexed: connection
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
            .context("failed to count immutable index nodes")?,
        links_indexed: connection
            .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
            .context("failed to count immutable index links")?,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use rusqlite::Connection;
    use slipbox_index::scan_source;

    use super::*;

    #[test]
    fn clones_a_current_wal_index_without_changing_the_source() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let source_path = workspace.path().join("source.sqlite3");
        let destination_path = workspace.path().join("destination.sqlite3");
        let mut source = Database::open(&source_path)?;
        source.sync_file_index(&scan_source(
            "alpha.org",
            "#+title: Alpha\n:PROPERTIES:\n:ID: alpha\n:END:\n",
        ))?;

        Database::clone_current_index(&source_path, &destination_path)?;

        let destination = Database::open(&destination_path)?;
        assert_eq!(destination.stats()?, source.stats()?);
        assert_eq!(
            destination.node_from_id("alpha")?,
            source.node_from_id("alpha")?
        );
        Ok(())
    }

    #[test]
    fn refuses_an_old_schema_without_creating_a_clone_or_rebuilding_the_base() -> Result<()> {
        let workspace = tempfile::tempdir()?;
        let source_path = workspace.path().join("old.sqlite3");
        let destination_path = workspace.path().join("destination.sqlite3");
        let connection = Connection::open(&source_path)?;
        connection.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION - 1)?;
        drop(connection);

        Database::clone_current_index(&source_path, &destination_path)
            .expect_err("an old schema needs an explicit rebuild");

        assert!(!destination_path.exists());
        let bytes = fs::metadata(&source_path)?.len();
        let reopened = Connection::open_with_flags(&source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version: i32 = reopened.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(version, INDEX_SCHEMA_VERSION - 1);
        assert_eq!(fs::metadata(&source_path)?.len(), bytes);
        Ok(())
    }
}

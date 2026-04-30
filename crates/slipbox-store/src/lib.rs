mod admin;
mod agenda;
mod backlinks;
mod comparison;
mod exploration;
mod files;
mod forward_links;
mod graph;
mod links;
mod nodes;
mod occurrences;
mod refs;
mod schema;
mod sync;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

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

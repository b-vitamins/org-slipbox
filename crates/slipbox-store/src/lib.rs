mod admin;
mod agenda;
mod backlinks;
mod files;
mod forward_links;
mod graph;
mod links;
mod nodes;
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

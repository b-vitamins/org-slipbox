use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use slipbox_core::NodeRecord;
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};
use slipbox_store::Database;

use crate::server::rpc::internal_error;

pub(super) struct ServerState {
    pub(super) root: PathBuf,
    pub(super) db_path: PathBuf,
    pub(super) database: Database,
}

impl ServerState {
    pub(super) fn new(root: PathBuf, db_path: PathBuf) -> Result<Self> {
        let database = Database::open(&db_path)?;
        Ok(Self {
            root,
            db_path,
            database,
        })
    }
}

pub(super) fn sync_one_path(state: &mut ServerState, path: &Path) -> Result<(), JsonRpcError> {
    let indexed_file = slipbox_index::scan_path(&state.root, path)
        .map_err(|error| internal_error(error.context("failed to scan updated file")))?;
    state
        .database
        .sync_file_index(&indexed_file)
        .map_err(|error| {
            internal_error(error.context("failed to sync updated file into SQLite"))
        })?;
    Ok(())
}

pub(super) fn sync_changed_paths(
    state: &mut ServerState,
    paths: &[PathBuf],
) -> Result<(), JsonRpcError> {
    for path in paths {
        sync_one_path(state, path)?;
    }
    Ok(())
}

pub(super) fn remove_deleted_paths(
    state: &mut ServerState,
    paths: &[PathBuf],
) -> Result<(), JsonRpcError> {
    for path in paths {
        let relative_path = path
            .strip_prefix(&state.root)
            .map_err(|_| {
                internal_error(anyhow!(
                    "{} is not under {}",
                    path.display(),
                    state.root.display()
                ))
            })?
            .to_string_lossy()
            .replace('\\', "/");
        state
            .database
            .remove_file_index(&relative_path)
            .map_err(|error| {
                internal_error(error.context("failed to remove file from SQLite index"))
            })?;
    }
    Ok(())
}

pub(super) fn read_required_node(
    state: &mut ServerState,
    node_key: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_by_key(node_key)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            internal_error(anyhow!(
                "{description} {node_key} was not found after indexing"
            ))
        })
}

pub(super) fn read_required_node_by_id(
    state: &mut ServerState,
    explicit_id: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_from_id(explicit_id)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            internal_error(anyhow!(
                "{description} {explicit_id} was not found after indexing"
            ))
        })
}

pub(super) fn read_known_node(
    state: &mut ServerState,
    node_key: &str,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    state
        .database
        .node_by_key(node_key)
        .map_err(|error| internal_error(error.context(format!("failed to fetch {description}"))))?
        .ok_or_else(|| {
            JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                "unknown {description}: {node_key}"
            )))
        })
}

pub(super) fn resolve_index_path(root: &Path, file_path: &str) -> Result<(String, PathBuf)> {
    let candidate = PathBuf::from(file_path);
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        root.join(candidate)
    };
    let relative = absolute
        .strip_prefix(root)
        .map_err(|_| anyhow!("{} is not under {}", absolute.display(), root.display()))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok((relative, absolute))
}

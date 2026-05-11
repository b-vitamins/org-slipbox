use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use slipbox_core::{AnchorRecord, CaptureTemplatePreviewResult, NodeRecord, PreviewNodeRecord};
use slipbox_index::DiscoveryPolicy;
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};
use slipbox_store::Database;
use slipbox_write::{CaptureOutcome, CapturePreviewOutcome, RegionRewriteOutcome, RewriteOutcome};

use crate::server::rpc::internal_error;

pub(super) struct ServerState {
    pub(super) root: PathBuf,
    pub(super) db_path: PathBuf,
    pub(super) workflow_dirs: Vec<PathBuf>,
    pub(super) discovery: DiscoveryPolicy,
    pub(super) database: Database,
}

impl ServerState {
    pub(super) fn new(
        root: PathBuf,
        db_path: PathBuf,
        workflow_dirs: Vec<PathBuf>,
        discovery: DiscoveryPolicy,
    ) -> Result<Self> {
        let database = Database::open(&db_path)?;
        Ok(Self {
            root,
            db_path,
            workflow_dirs,
            discovery,
            database,
        })
    }

    pub(super) fn resolve_index_path(&self, file_path: &str) -> Result<(String, PathBuf)> {
        let candidate = PathBuf::from(file_path);
        let absolute = if candidate.is_absolute() {
            candidate
        } else {
            self.root.join(candidate)
        };
        let relative = self.relative_root_path(&absolute)?;
        Ok((relative, absolute))
    }

    pub(super) fn sync_path(&mut self, path: &Path) -> Result<(), JsonRpcError> {
        let indexed_file = slipbox_index::scan_path_with_policy(&self.root, path, &self.discovery)
            .map_err(|error| internal_error(error.context("failed to scan updated file")))?;
        self.database
            .sync_file_index(&indexed_file)
            .map_err(|error| {
                internal_error(error.context("failed to sync updated file into SQLite"))
            })?;
        Ok(())
    }

    pub(super) fn preview_capture(
        &self,
        outcome: &CapturePreviewOutcome,
    ) -> Result<CaptureTemplatePreviewResult, JsonRpcError> {
        let indexed = slipbox_index::scan_source(&outcome.relative_path, &outcome.content);
        let node = indexed
            .nodes
            .into_iter()
            .find(|candidate| candidate.node_key == outcome.node_key)
            .map(PreviewNodeRecord::from)
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "captured preview node {} was not found in rendered output",
                    outcome.node_key
                ))
            })?;
        Ok(CaptureTemplatePreviewResult {
            file_path: outcome.relative_path.clone(),
            content: outcome.content.clone(),
            preview_node: node,
        })
    }

    pub(super) fn sync_region_rewrite(
        &mut self,
        outcome: &RegionRewriteOutcome,
    ) -> Result<(), JsonRpcError> {
        self.reconcile_paths(&outcome.changed_paths, &outcome.removed_paths)
    }

    fn sync_paths(&mut self, paths: &[PathBuf]) -> Result<(), JsonRpcError> {
        for path in paths {
            self.sync_path(path)?;
        }
        Ok(())
    }

    fn remove_deleted_paths(&mut self, paths: &[PathBuf]) -> Result<(), JsonRpcError> {
        for path in paths {
            let relative_path = self
                .relative_root_path(path)
                .map_err(|error| internal_error(error.context("failed to resolve deleted path")))?;
            self.database
                .remove_file_index(&relative_path)
                .map_err(|error| {
                    internal_error(error.context("failed to remove file from SQLite index"))
                })?;
        }
        Ok(())
    }

    fn reconcile_paths(
        &mut self,
        changed_paths: &[PathBuf],
        removed_paths: &[PathBuf],
    ) -> Result<(), JsonRpcError> {
        self.sync_paths(changed_paths)?;
        self.remove_deleted_paths(removed_paths)
    }

    pub(super) fn require_note(
        &mut self,
        node_key: &str,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.database
            .note_by_key(node_key)
            .map_err(|error| {
                internal_error(error.context(format!("failed to fetch {description}")))
            })?
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "{description} {node_key} was not found after indexing"
                ))
            })
    }

    pub(super) fn require_note_by_id(
        &mut self,
        explicit_id: &str,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.database
            .node_from_id(explicit_id)
            .map_err(|error| {
                internal_error(error.context(format!("failed to fetch {description}")))
            })?
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "{description} {explicit_id} was not found after indexing"
                ))
            })
    }

    pub(super) fn known_note(
        &mut self,
        node_key: &str,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.database
            .note_by_key(node_key)
            .map_err(|error| {
                internal_error(error.context(format!("failed to fetch {description}")))
            })?
            .ok_or_else(|| {
                JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                    "unknown {description}: {node_key}"
                )))
            })
    }

    pub(super) fn known_note_for_node_or_anchor(
        &mut self,
        node_key: &str,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        if let Some(note) = self.database.note_by_key(node_key).map_err(|error| {
            internal_error(error.context(format!("failed to fetch {description}")))
        })? {
            return Ok(note);
        }

        let anchor = self.known_anchor(node_key, description)?;
        self.database
            .note_for_anchor(&anchor)
            .map_err(|error| {
                internal_error(
                    error.context(format!("failed to resolve owner note for {description}")),
                )
            })?
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "owner note for {description} {node_key} was not found after indexing"
                ))
            })
    }

    pub(super) fn require_anchor(
        &mut self,
        node_key: &str,
        description: &str,
    ) -> Result<AnchorRecord, JsonRpcError> {
        self.database
            .anchor_by_key(node_key)
            .map_err(|error| {
                internal_error(error.context(format!("failed to fetch {description}")))
            })?
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "{description} {node_key} was not found after indexing"
                ))
            })
    }

    pub(super) fn known_anchor(
        &mut self,
        node_key: &str,
        description: &str,
    ) -> Result<AnchorRecord, JsonRpcError> {
        self.database
            .anchor_by_key(node_key)
            .map_err(|error| {
                internal_error(error.context(format!("failed to fetch {description}")))
            })?
            .ok_or_else(|| {
                JsonRpcError::new(JsonRpcErrorObject::invalid_request(format!(
                    "unknown {description}: {node_key}"
                )))
            })
    }

    pub(super) fn sync_capture(
        &mut self,
        outcome: &CaptureOutcome,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.sync_path(&outcome.absolute_path)?;
        self.require_note(&outcome.node_key, description)
    }

    pub(super) fn sync_capture_anchor(
        &mut self,
        outcome: &CaptureOutcome,
        description: &str,
    ) -> Result<AnchorRecord, JsonRpcError> {
        self.sync_path(&outcome.absolute_path)?;
        self.require_anchor(&outcome.node_key, description)
    }

    pub(super) fn sync_path_and_read_node(
        &mut self,
        path: &Path,
        node_key: &str,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.sync_path(path)?;
        self.require_note(node_key, description)
    }

    pub(super) fn sync_rewrite(
        &mut self,
        outcome: &RewriteOutcome,
        description: &str,
    ) -> Result<NodeRecord, JsonRpcError> {
        self.reconcile_paths(&outcome.changed_paths, &outcome.removed_paths)?;
        self.require_note_by_id(&outcome.explicit_id, description)
    }

    fn relative_root_path(&self, path: &Path) -> Result<String> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| anyhow!("{} is not under {}", path.display(), self.root.display()))?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }
}

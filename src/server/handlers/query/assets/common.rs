use serde::Serialize;
use slipbox_core::{ExplorationLens, NodeRecord, WorkflowResolveTarget};
use slipbox_rpc::JsonRpcError;

use super::super::common::invalid_request;
use crate::server::rpc::internal_error;
use crate::server::state::ServerState;
use crate::server::workflows::{WorkflowCatalog, discover_workflow_catalog};

pub(super) fn discover_server_workflow_catalog(
    state: &ServerState,
) -> Result<WorkflowCatalog, JsonRpcError> {
    let packs = state
        .database
        .list_workbench_packs()
        .map_err(|error| internal_error(error.context("failed to list workbench packs")))?;
    Ok(discover_workflow_catalog(
        &state.root,
        &state.workflow_dirs,
        &packs,
    ))
}

fn workflow_lens_accepts_anchor_focus(lens: ExplorationLens) -> bool {
    matches!(
        lens,
        ExplorationLens::Refs | ExplorationLens::Time | ExplorationLens::Tasks
    )
}

pub(super) fn resolve_workflow_note_target(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    match target {
        WorkflowResolveTarget::Id { id } => state
            .database
            .node_from_id(id)
            .map_err(|error| {
                internal_error(error.context(format!("failed to resolve {description}")))
            })?
            .ok_or_else(|| invalid_request(format!("unknown {description}: {id}"))),
        WorkflowResolveTarget::Title { title } => {
            let matches = state
                .database
                .node_from_title_or_alias(title, false)
                .map_err(|error| {
                    internal_error(error.context(format!("failed to resolve {description}")))
                })?;
            if matches.len() > 1 {
                return Err(invalid_request(format!("multiple nodes match {title}")));
            }
            matches
                .into_iter()
                .next()
                .ok_or_else(|| invalid_request(format!("unknown {description}: {title}")))
        }
        WorkflowResolveTarget::Reference { reference } => state
            .database
            .node_from_ref(reference)
            .map_err(|error| {
                internal_error(error.context(format!("failed to resolve {description}")))
            })?
            .ok_or_else(|| invalid_request(format!("unknown {description}: {reference}"))),
        WorkflowResolveTarget::NodeKey { node_key } => state.known_note(node_key, description),
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime resolution unexpectedly"
        ))),
    }
}

pub(super) fn resolve_workflow_note_target_from_focus(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    description: &str,
) -> Result<NodeRecord, JsonRpcError> {
    match target {
        WorkflowResolveTarget::NodeKey { node_key } => {
            state.known_note_for_node_or_anchor(node_key, description)
        }
        WorkflowResolveTarget::Id { .. }
        | WorkflowResolveTarget::Title { .. }
        | WorkflowResolveTarget::Reference { .. } => {
            resolve_workflow_note_target(state, target, description)
        }
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime note resolution unexpectedly"
        ))),
    }
}

pub(super) fn resolve_workflow_focus_target(
    state: &mut ServerState,
    target: &WorkflowResolveTarget,
    lens: ExplorationLens,
    description: &str,
) -> Result<String, JsonRpcError> {
    match target {
        WorkflowResolveTarget::NodeKey { node_key } if workflow_lens_accepts_anchor_focus(lens) => {
            if state
                .database
                .anchor_by_key(node_key)
                .map_err(|error| {
                    internal_error(error.context(format!("failed to resolve {description}")))
                })?
                .is_some()
            {
                Ok(node_key.clone())
            } else {
                state
                    .known_note(node_key, description)
                    .map(|note| note.node_key)
            }
        }
        WorkflowResolveTarget::NodeKey { node_key } => state
            .known_note_for_node_or_anchor(node_key, description)
            .map(|note| note.node_key),
        WorkflowResolveTarget::Id { .. }
        | WorkflowResolveTarget::Title { .. }
        | WorkflowResolveTarget::Reference { .. } => {
            resolve_workflow_note_target(state, target, description).map(|note| note.node_key)
        }
        WorkflowResolveTarget::Input { .. } => Err(internal_error(anyhow::anyhow!(
            "workflow input reference reached runtime focus resolution unexpectedly"
        ))),
    }
}

pub(super) fn stable_json_fingerprint<T: Serialize>(value: &T) -> Result<String, JsonRpcError> {
    let bytes = serde_json::to_vec(value).map_err(|error| {
        internal_error(anyhow::anyhow!(
            "failed to serialize review source: {error}"
        ))
    })?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

pub(super) fn reject_existing_review_run(
    state: &ServerState,
    review_id: &str,
) -> Result<(), JsonRpcError> {
    if state
        .database
        .review_run(review_id)
        .map_err(|error| internal_error(error.context("failed to load review run")))?
        .is_some()
    {
        return Err(invalid_request(format!(
            "review run already exists: {review_id}"
        )));
    }

    Ok(())
}

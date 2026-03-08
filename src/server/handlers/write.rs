use slipbox_core::{
    AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    CaptureNodeParams, CaptureTemplateParams, CaptureTemplatePreviewParams,
    CaptureTemplatePreviewResult, EnsureFileNodeParams, EnsureNodeIdParams, ExtractSubtreeParams,
    RefileRegionParams, RefileSubtreeParams, RewriteFileParams, UpdateNodeMetadataParams,
};
use slipbox_rpc::JsonRpcError;

use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn capture_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: CaptureNodeParams = parse_params(params)?;
    let refs = params.normalized_refs();
    let captured = match params.file_path.as_deref() {
        Some(file_path) => {
            if let Some(head) = params.head.as_deref() {
                slipbox_write::capture_file_note_at_with_head_and_refs(
                    &state.root,
                    file_path,
                    &params.title,
                    head,
                    &refs,
                )
            } else {
                slipbox_write::capture_file_note_at_with_refs(
                    &state.root,
                    file_path,
                    &params.title,
                    &refs,
                )
            }
        }
        None => slipbox_write::capture_file_note_with_refs(&state.root, &params.title, &refs),
    }
    .map_err(|error| internal_error(error.context("failed to capture node")))?;
    to_value(state.sync_capture(&captured, "captured node")?)
}

pub(crate) fn capture_template(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let mut params: CaptureTemplateParams = parse_params(params)?;
    if let Some(file_path) = params.file_path.as_deref() {
        let (relative_path, _) = state
            .resolve_index_path(file_path)
            .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
        params.file_path = Some(relative_path);
    }
    let target = match params.node_key.as_deref() {
        Some(node_key) => Some(state.known_node(node_key, "target node")?),
        None => None,
    };
    let captured = slipbox_write::capture_template(&state.root, target.as_ref(), &params)
        .map_err(|error| internal_error(error.context("failed to capture template")))?;
    to_value(state.sync_capture(&captured, "captured template")?)
}

pub(crate) fn capture_template_preview(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let mut params: CaptureTemplatePreviewParams = parse_params(params)?;
    if let Some(file_path) = params.capture.file_path.as_deref() {
        let (relative_path, _) = state
            .resolve_index_path(file_path)
            .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
        params.capture.file_path = Some(relative_path);
    }
    let target = match params.capture.node_key.as_deref() {
        Some(node_key) => Some(state.known_node(node_key, "target node")?),
        None => None,
    };
    let preview = slipbox_write::preview_capture_template(
        &state.root,
        target.as_ref(),
        &params.capture,
        params.source_override.as_deref(),
        params.ensure_node_id,
    )
    .map_err(|error| internal_error(error.context("failed to preview capture template")))?;
    let indexed = slipbox_index::scan_source(&preview.relative_path, &preview.content);
    let node = indexed
        .nodes
        .into_iter()
        .find(|candidate| candidate.node_key == preview.node_key)
        .map(Into::into)
        .ok_or_else(|| {
            internal_error(anyhow::anyhow!(
                "captured preview node {} was not found in rendered output",
                preview.node_key
            ))
        })?;
    to_value(CaptureTemplatePreviewResult {
        file_path: preview.relative_path,
        content: preview.content,
        node,
    })
}

pub(crate) fn ensure_file_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureFileNodeParams = parse_params(params)?;
    let ensured = slipbox_write::ensure_file_note(&state.root, &params.file_path, &params.title)
        .map_err(|error| internal_error(error.context("failed to ensure file node")))?;
    to_value(state.sync_capture(&ensured, "ensured file node")?)
}

pub(crate) fn append_heading(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingParams = parse_params(params)?;
    let captured = slipbox_write::append_heading(
        &state.root,
        &params.file_path,
        &params.title,
        &params.heading,
        params.normalized_level(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading")))?;
    to_value(state.sync_capture(&captured, "captured heading")?)
}

pub(crate) fn append_heading_to_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingToNodeParams = parse_params(params)?;
    let target = state.known_node(&params.node_key, "node")?;
    let captured = slipbox_write::append_heading_to_node(&state.root, &target, &params.heading)
        .map_err(|error| internal_error(error.context("failed to append heading to node")))?;
    to_value(state.sync_capture(&captured, "captured heading")?)
}

pub(crate) fn append_heading_at_outline_path(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingAtOutlinePathParams = parse_params(params)?;
    let captured = slipbox_write::append_heading_at_outline_path(
        &state.root,
        &params.file_path,
        &params.heading,
        &params.normalized_outline_path(),
        params.head.as_deref(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading at outline path")))?;
    to_value(state.sync_capture(&captured, "captured heading")?)
}

pub(crate) fn ensure_node_id(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureNodeIdParams = parse_params(params)?;
    let node = state.known_node(&params.node_key, "node")?;

    if node.explicit_id.is_none() {
        let updated_path = slipbox_write::ensure_node_id(&state.root, &node)
            .map_err(|error| internal_error(error.context("failed to assign node ID")))?;
        state.sync_path(&updated_path)?;
    }

    to_value(state.require_node(&params.node_key, "updated node")?)
}

pub(crate) fn update_node_metadata(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: UpdateNodeMetadataParams = parse_params(params)?;
    let node = state.known_node(&params.node_key, "node")?;
    let updated_path = slipbox_write::update_node_metadata(
        &state.root,
        &node,
        &slipbox_write::MetadataUpdate {
            aliases: params.normalized_aliases(),
            refs: params.normalized_refs(),
            tags: params.normalized_tags(),
        },
    )
    .map_err(|error| internal_error(error.context("failed to update node metadata")))?;
    to_value(state.sync_path_and_read_node(&updated_path, &params.node_key, "updated node")?)
}

pub(crate) fn refile_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RefileSubtreeParams = parse_params(params)?;
    let source = state.known_node(&params.source_node_key, "source node")?;
    let target = state.known_node(&params.target_node_key, "target node")?;
    let outcome = slipbox_write::refile_subtree(&state.root, &source, &target)
        .map_err(|error| internal_error(error.context("failed to refile subtree")))?;
    to_value(state.sync_rewrite(&outcome, "refiled node")?)
}

pub(crate) fn refile_region(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RefileRegionParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let target = state.known_node(&params.target_node_key, "target node")?;
    let (start, end) = params.normalized_range();
    let outcome = slipbox_write::refile_region(&state.root, &relative_path, start, end, &target)
        .map_err(|error| internal_error(error.context("failed to refile region")))?;
    state.sync_paths(&outcome.changed_paths)?;
    state.remove_deleted_paths(&outcome.removed_paths)?;
    to_value(serde_json::json!({ "refiled": true }))
}

pub(crate) fn extract_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExtractSubtreeParams = parse_params(params)?;
    let source = state.known_node(&params.source_node_key, "source node")?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::extract_subtree(&state.root, &source, &relative_path)
        .map_err(|error| internal_error(error.context("failed to extract subtree")))?;
    to_value(state.sync_rewrite(&outcome, "extracted node")?)
}

pub(crate) fn promote_entire_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RewriteFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::promote_entire_file(&state.root, &relative_path)
        .map_err(|error| internal_error(error.context("failed to promote file node")))?;
    to_value(state.sync_path_and_read_node(
        &absolute_path,
        &outcome.node_key,
        "promoted file node",
    )?)
}

pub(crate) fn demote_entire_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RewriteFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::demote_entire_file(&state.root, &relative_path)
        .map_err(|error| internal_error(error.context("failed to demote file node")))?;
    to_value(state.sync_path_and_read_node(
        &absolute_path,
        &outcome.node_key,
        "demoted file node",
    )?)
}

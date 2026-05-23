use slipbox_core::{
    AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    CaptureNodeParams, CaptureTemplateParams, CaptureTemplatePreviewParams, EnsureFileNodeParams,
    EnsureNodeIdParams, UpdateNodeMetadataParams,
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
            let (relative_path, _) = state
                .resolve_index_path(file_path)
                .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
            if let Some(head) = params.head.as_deref() {
                slipbox_write::capture_file_note_at_with_head_and_refs(
                    &state.root,
                    &relative_path,
                    &params.title,
                    head,
                    &refs,
                )
            } else {
                slipbox_write::capture_file_note_at_with_refs(
                    &state.root,
                    &relative_path,
                    &params.title,
                    &refs,
                )
            }
        }
        None => slipbox_write::capture_file_note_with_refs(&state.root, &params.title, &refs),
    }
    .map_err(|error| internal_error(error.context("failed to create file note")))?;
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
        Some(node_key) => Some(state.known_note(node_key, "target node")?),
        None => None,
    };
    let captured = slipbox_write::capture_template(&state.root, target.as_ref(), &params)
        .map_err(|error| internal_error(error.context("failed to capture template")))?;
    to_value(state.sync_capture_anchor(&captured, "captured template")?)
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
        Some(node_key) => Some(state.known_note(node_key, "target node")?),
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
    to_value(state.preview_capture(&preview)?)
}

pub(crate) fn ensure_file_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureFileNodeParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let ensured = slipbox_write::ensure_file_note(&state.root, &relative_path, &params.title)
        .map_err(|error| internal_error(error.context("failed to ensure file node")))?;
    to_value(state.sync_capture(&ensured, "ensured file node")?)
}

pub(crate) fn append_heading(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let captured = slipbox_write::append_heading(
        &state.root,
        &relative_path,
        &params.title,
        &params.heading,
        params.normalized_level(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading")))?;
    to_value(state.sync_capture_anchor(&captured, "captured heading")?)
}

pub(crate) fn append_heading_to_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingToNodeParams = parse_params(params)?;
    let target = state.known_note(&params.node_key, "node")?;
    let captured = slipbox_write::append_heading_to_node(&state.root, &target, &params.heading)
        .map_err(|error| internal_error(error.context("failed to append heading to node")))?;
    to_value(state.sync_capture_anchor(&captured, "captured heading")?)
}

pub(crate) fn append_heading_at_outline_path(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AppendHeadingAtOutlinePathParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let captured = slipbox_write::append_heading_at_outline_path(
        &state.root,
        &relative_path,
        &params.heading,
        &params.normalized_outline_path(),
        params.head.as_deref(),
    )
    .map_err(|error| internal_error(error.context("failed to append heading at outline path")))?;
    to_value(state.sync_capture_anchor(&captured, "captured heading")?)
}

pub(crate) fn ensure_node_id(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: EnsureNodeIdParams = parse_params(params)?;
    let node = state.known_anchor(&params.node_key, "node")?;

    if node.explicit_id.is_none() {
        let updated_path = slipbox_write::ensure_node_id(&state.root, &node)
            .map_err(|error| internal_error(error.context("failed to assign node ID")))?;
        state.sync_path(&updated_path)?;
    }

    to_value(state.require_anchor(&params.node_key, "updated node")?)
}

pub(crate) fn update_node_metadata(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: UpdateNodeMetadataParams = parse_params(params)?;
    let node = state.known_note(&params.node_key, "node")?;
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

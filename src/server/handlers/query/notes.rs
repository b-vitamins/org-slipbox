use std::collections::BTreeSet;
use std::fs;

use slipbox_core::{
    AnchorFromKeyParams, AnchorRecord, NodeAtPointParams, NodeFromIdParams, NodeFromKeyParams,
    NodeFromTitleOrAliasParams, NodeKind, NoteContextParams, NoteContextResult, RandomNodeResult,
    ReadFileSourceParams, ReadFileSourceResult, ReadNodeSourceParams, ReadNodeSourceResult,
    SearchNodesParams, SearchNodesResult, SourceSlice,
};
use slipbox_rpc::JsonRpcError;

use crate::server::rpc::{
    internal_error, invalid_params, not_found, parse_params, path_denied, to_value,
};
use crate::server::state::ServerState;

pub(crate) fn search_nodes(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchNodesParams = parse_params(params)?;
    let nodes = state
        .database
        .search_nodes(
            &params.query,
            params.normalized_limit(),
            params.sort.clone(),
        )
        .map_err(|error| internal_error(error.context("failed to query nodes")))?;
    let nodes = live_nodes(state, nodes)?;
    to_value(SearchNodesResult { nodes })
}

pub(crate) fn random_node(
    state: &mut ServerState,
    _params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    loop {
        let node = state
            .database
            .random_node()
            .map_err(|error| internal_error(error.context("failed to query random node")))?;
        match node {
            Some(node) => {
                if let Some(node) = live_node(state, node)? {
                    return to_value(RandomNodeResult { node: Some(node) });
                }
            }
            None => return to_value(RandomNodeResult { node: None }),
        }
    }
}

pub(crate) fn node_from_id(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromIdParams = parse_params(params)?;
    let node = state
        .database
        .node_from_id(&params.id)
        .map_err(|error| internal_error(error.context("failed to resolve node ID")))?;
    let node = live_optional_node(state, node)?;
    to_value(node)
}

pub(crate) fn node_from_key(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromKeyParams = parse_params(params)?;
    let node = state
        .database
        .note_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to resolve node key")))?;
    let node = live_optional_node(state, node)?;
    to_value(node)
}

pub(crate) fn node_from_title_or_alias(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeFromTitleOrAliasParams = parse_params(params)?;
    let matches = state
        .database
        .node_from_title_or_alias(&params.title_or_alias, params.nocase)
        .map_err(|error| internal_error(error.context("failed to resolve node title or alias")))?;
    let matches = live_nodes(state, matches)?;
    if matches.len() > 1 {
        return Err(invalid_params(format!(
            "multiple nodes match {}",
            params.title_or_alias
        )));
    }
    to_value(matches.into_iter().next())
}

fn live_optional_node(
    state: &mut ServerState,
    node: Option<slipbox_core::NodeRecord>,
) -> Result<Option<slipbox_core::NodeRecord>, JsonRpcError> {
    node.map(|node| live_node(state, node))
        .transpose()
        .map(Option::flatten)
}

fn live_node(
    state: &mut ServerState,
    node: slipbox_core::NodeRecord,
) -> Result<Option<slipbox_core::NodeRecord>, JsonRpcError> {
    if state.indexed_file_is_live(&node.file_path) {
        Ok(Some(node))
    } else {
        state.remove_indexed_file_path(&node.file_path, "missing indexed node file")?;
        Ok(None)
    }
}

fn live_nodes(
    state: &mut ServerState,
    nodes: Vec<slipbox_core::NodeRecord>,
) -> Result<Vec<slipbox_core::NodeRecord>, JsonRpcError> {
    let mut live = Vec::with_capacity(nodes.len());
    let mut missing = BTreeSet::new();
    for node in nodes {
        if state.indexed_file_is_live(&node.file_path) {
            live.push(node);
        } else {
            missing.insert(node.file_path);
        }
    }
    for file_path in missing {
        state.remove_indexed_file_path(&file_path, "missing indexed node file")?;
    }
    Ok(live)
}

pub(crate) fn anchor_from_key(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: AnchorFromKeyParams = parse_params(params)?;
    let anchor = state
        .database
        .anchor_by_key(&params.node_key)
        .map_err(|error| internal_error(error.context("failed to resolve anchor key")))?;
    to_value(anchor)
}

pub(crate) fn read_file_source(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReadFileSourceParams = parse_params(params)?;
    let (relative_path, source) = read_source_file(state, &params.file_path)?;
    to_value(ReadFileSourceResult {
        source: source_slice(
            relative_path,
            &source,
            params.normalized_start_line(),
            params.normalized_max_lines(),
        ),
    })
}

pub(crate) fn read_node_source(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ReadNodeSourceParams = parse_params(params)?;
    let anchor = state.known_anchor(&params.node_key, "source anchor")?;
    let result = read_anchor_source(
        state,
        anchor,
        params.normalized_context_before(),
        params.normalized_context_after(),
        params.normalized_max_lines(),
    )?;
    to_value(result)
}

pub(crate) fn note_context(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NoteContextParams = parse_params(params)?;
    let note = state.known_note_for_node_or_anchor(&params.node_key, "context note")?;
    let source = read_anchor_source(
        state,
        AnchorRecord::from(note.clone()),
        params.normalized_source_context_before(),
        params.normalized_source_context_after(),
        params.normalized_source_max_lines(),
    )?;
    let relation_limit = params.normalized_relation_limit();
    let backlinks = state
        .database
        .backlinks(&note.node_key, relation_limit, true)
        .map_err(|error| internal_error(error.context("failed to query context backlinks")))?;
    let forward_links = state
        .database
        .forward_links(&note.node_key, relation_limit, true)
        .map_err(|error| internal_error(error.context("failed to query context forward links")))?;
    to_value(NoteContextResult {
        note,
        source: source.source,
        node_start_line: source.node_start_line,
        node_line_count: source.node_line_count,
        backlinks,
        forward_links,
    })
}

pub(crate) fn node_at_point(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeAtPointParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| path_denied(error.to_string()))?;
    let node = state
        .database
        .node_at_point(&relative_path, params.normalized_line())
        .map_err(|error| internal_error(error.context("failed to resolve node at point")))?;
    to_value(node)
}

pub(crate) fn anchor_at_point(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeAtPointParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| path_denied(error.to_string()))?;
    let anchor = state
        .database
        .anchor_at_point(&relative_path, params.normalized_line())
        .map_err(|error| internal_error(error.context("failed to resolve anchor at point")))?;
    to_value(anchor)
}

fn read_source_file(
    state: &ServerState,
    file_path: &str,
) -> Result<(String, String), JsonRpcError> {
    let (relative_path, absolute_path) = state
        .resolve_index_path(file_path)
        .map_err(|error| path_denied(error.to_string()))?;
    if !absolute_path.is_file() {
        return Err(not_found(format!("source file not found: {relative_path}")));
    }
    if !state.discovery.matches_path(&state.root, &absolute_path) {
        return Err(invalid_params(format!(
            "source file is not eligible for indexing: {relative_path}"
        )));
    }
    let source = fs::read_to_string(&absolute_path).map_err(|error| {
        internal_error(
            anyhow::anyhow!(error).context(format!("failed to read source file {relative_path}")),
        )
    })?;
    Ok((relative_path, source))
}

fn read_anchor_source(
    state: &mut ServerState,
    anchor: AnchorRecord,
    context_before: u32,
    context_after: u32,
    max_lines: usize,
) -> Result<ReadNodeSourceResult, JsonRpcError> {
    let (relative_path, source) = read_source_file(state, &anchor.file_path)?;
    let anchors = state
        .database
        .anchors_in_file(&relative_path)
        .map_err(|error| internal_error(error.context("failed to read source anchors")))?;
    let total_lines = source_line_count(&source);
    let (node_start_line, node_end_line) = anchor_line_extent(&anchor, &anchors, total_lines);
    let requested_start = node_start_line.saturating_sub(context_before).max(1);
    let requested_end = node_end_line
        .saturating_add(context_after)
        .min(total_lines.max(node_end_line));
    let requested_lines = requested_end
        .saturating_sub(requested_start)
        .saturating_add(1)
        .min(max_lines as u32) as usize;
    Ok(ReadNodeSourceResult {
        anchor,
        source: source_slice(relative_path, &source, requested_start, requested_lines),
        node_start_line,
        node_line_count: node_end_line
            .saturating_sub(node_start_line)
            .saturating_add(1),
    })
}

fn anchor_line_extent(
    anchor: &AnchorRecord,
    anchors: &[AnchorRecord],
    total_lines: u32,
) -> (u32, u32) {
    if matches!(anchor.kind, NodeKind::File) {
        return (1, total_lines.max(1));
    }

    let end_line = anchors
        .iter()
        .find(|candidate| candidate.line > anchor.line && candidate.level <= anchor.level)
        .map(|candidate| candidate.line.saturating_sub(1))
        .unwrap_or(total_lines)
        .max(anchor.line);
    (anchor.line.max(1), end_line)
}

fn source_slice(file_path: String, source: &str, start_line: u32, max_lines: usize) -> SourceSlice {
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let total_lines = lines.len() as u32;
    if total_lines == 0 {
        return SourceSlice {
            file_path,
            start_line: 1,
            line_count: 0,
            total_lines,
            content: String::new(),
            truncated_before: false,
            truncated_after: false,
        };
    }

    let start_line = start_line.max(1);
    if start_line > total_lines {
        return SourceSlice {
            file_path,
            start_line,
            line_count: 0,
            total_lines,
            content: String::new(),
            truncated_before: true,
            truncated_after: false,
        };
    }

    let start_index = (start_line - 1) as usize;
    let end_index = lines.len().min(start_index.saturating_add(max_lines));
    SourceSlice {
        file_path,
        start_line,
        line_count: (end_index - start_index) as u32,
        total_lines,
        content: lines[start_index..end_index].concat(),
        truncated_before: start_index > 0,
        truncated_after: end_index < lines.len(),
    }
}

fn source_line_count(source: &str) -> u32 {
    source.split_inclusive('\n').count() as u32
}

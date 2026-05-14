use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::anyhow;
use slipbox_core::{
    FileDiagnosticIssue, FileDiagnostics, FileDiagnosticsParams, FileDiagnosticsResult,
    IndexDiagnostics, IndexDiagnosticsResult, IndexFileParams, IndexFileResult, IndexedFilesResult,
    NodeDiagnosticIssue, NodeDiagnostics, NodeDiagnosticsParams, NodeDiagnosticsResult, PingInfo,
    SearchFilesParams, SearchFilesResult, StatusInfo,
};
use slipbox_rpc::JsonRpcError;

use super::common::invalid_request;
use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn ping(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    to_value(PingInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        root: state.root.display().to_string(),
        db: state.db_path.display().to_string(),
    })
}

pub(crate) fn status(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let stats = state
        .database
        .stats()
        .map_err(|error| internal_error(error.context("failed to read index statistics")))?;
    to_value(StatusInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        root: state.root.display().to_string(),
        db: state.db_path.display().to_string(),
        files_indexed: stats.files_indexed,
        nodes_indexed: stats.nodes_indexed,
        links_indexed: stats.links_indexed,
    })
}

pub(crate) fn index(state: &mut ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let files = slipbox_index::scan_root_with_policy(&state.root, &state.discovery)
        .map_err(|error| internal_error(error.context("failed to scan Org files")))?;
    let stats = state
        .database
        .sync_index(&files)
        .map_err(|error| internal_error(error.context("failed to update SQLite index")))?;
    to_value(stats)
}
pub(crate) fn indexed_files(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let files = state
        .database
        .indexed_files()
        .map_err(|error| internal_error(error.context("failed to read indexed files")))?;
    to_value(IndexedFilesResult { files })
}

pub(crate) fn search_files(
    state: &ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SearchFilesParams = parse_params(params)?;
    let files = state
        .database
        .search_files(&params.query, params.normalized_limit())
        .map_err(|error| internal_error(error.context("failed to query indexed files")))?;
    to_value(SearchFilesResult { files })
}

pub(crate) fn diagnose_file(
    state: &ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: FileDiagnosticsParams = parse_params(params)?;
    let diagnostic = file_diagnostics(state, &params.file_path)?;
    to_value(FileDiagnosticsResult { diagnostic })
}

pub(crate) fn diagnose_node(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: NodeDiagnosticsParams = parse_params(params)?;
    let node = state.known_anchor(&params.node_key, "diagnostic node")?;
    let file = file_diagnostics(state, &node.file_path)?;
    let line_present = file.exists
        && fs::read_to_string(state.root.join(&node.file_path))
            .map(|source| line_exists(&source, node.line))
            .unwrap_or(false);
    let mut issues = Vec::new();
    if !file.exists {
        issues.push(NodeDiagnosticIssue::SourceFileMissing);
    }
    if !file.eligible {
        issues.push(NodeDiagnosticIssue::SourceFileIneligible);
    }
    if !file.indexed {
        issues.push(NodeDiagnosticIssue::SourceFileUnindexed);
    }
    if !line_present {
        issues.push(NodeDiagnosticIssue::LineOutOfRange);
    }
    to_value(NodeDiagnosticsResult {
        diagnostic: NodeDiagnostics {
            node,
            file,
            line_present,
            issues,
        },
    })
}

pub(crate) fn diagnose_index(state: &ServerState) -> Result<serde_json::Value, JsonRpcError> {
    let eligible_files = eligible_files(state)?;
    let indexed_files = state
        .database
        .indexed_files()
        .map_err(|error| internal_error(error.context("failed to read indexed files")))?;
    let indexed_set = indexed_files.iter().cloned().collect::<HashSet<_>>();
    let eligible_set = eligible_files.iter().cloned().collect::<HashSet<_>>();
    let mut missing_from_index = eligible_files
        .iter()
        .filter(|file_path| !indexed_set.contains(*file_path))
        .cloned()
        .collect::<Vec<_>>();
    let mut indexed_but_missing = Vec::new();
    let mut indexed_but_ineligible = Vec::new();
    for file_path in &indexed_files {
        let absolute_path = state.root.join(file_path);
        if !absolute_path.is_file() {
            indexed_but_missing.push(file_path.clone());
        } else if !eligible_set.contains(file_path) {
            indexed_but_ineligible.push(file_path.clone());
        }
    }
    missing_from_index.sort();
    indexed_but_missing.sort();
    indexed_but_ineligible.sort();
    let status = state
        .database
        .stats()
        .map_err(|error| internal_error(error.context("failed to read index statistics")))?;
    let status_consistent = status.files_indexed as usize == indexed_files.len();
    let index_current = status_consistent
        && missing_from_index.is_empty()
        && indexed_but_missing.is_empty()
        && indexed_but_ineligible.is_empty();

    to_value(IndexDiagnosticsResult {
        diagnostic: IndexDiagnostics {
            root: state.root.display().to_string(),
            eligible_files,
            indexed_files,
            missing_from_index,
            indexed_but_missing,
            indexed_but_ineligible,
            status,
            status_consistent,
            index_current,
        },
    })
}

fn file_diagnostics(state: &ServerState, file_path: &str) -> Result<FileDiagnostics, JsonRpcError> {
    let (relative_path, absolute_path) = state
        .resolve_index_path(file_path)
        .map_err(|error| invalid_request(error.to_string()))?;
    let exists = absolute_path.is_file();
    let eligible = exists && state.discovery.matches_path(&state.root, &absolute_path);
    let index_record = state
        .database
        .file_record(&relative_path)
        .map_err(|error| internal_error(error.context("failed to read indexed file record")))?;
    let indexed = index_record.is_some();
    let mut issues = Vec::new();
    if exists && eligible && !indexed {
        issues.push(FileDiagnosticIssue::MissingFromIndex);
    }
    if indexed && !exists {
        issues.push(FileDiagnosticIssue::IndexedButMissing);
    }
    if indexed && exists && !eligible {
        issues.push(FileDiagnosticIssue::IndexedButIneligible);
    }

    Ok(FileDiagnostics {
        file_path: relative_path,
        absolute_path: absolute_path.display().to_string(),
        exists,
        eligible,
        indexed,
        index_record,
        issues,
    })
}

fn eligible_files(state: &ServerState) -> Result<Vec<String>, JsonRpcError> {
    let files = state
        .discovery
        .list_files(&state.root)
        .map_err(|error| internal_error(error.context("failed to discover eligible files")))?;
    let mut relative_files = files
        .iter()
        .map(|path| relative_root_path(&state.root, path))
        .collect::<Result<Vec<_>, _>>()?;
    relative_files.sort();
    Ok(relative_files)
}

fn relative_root_path(root: &Path, path: &Path) -> Result<String, JsonRpcError> {
    path.strip_prefix(root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            internal_error(anyhow!(
                "{} is not under {}",
                path.display(),
                root.display()
            ))
        })
}

fn line_exists(source: &str, line: u32) -> bool {
    line > 0 && source.lines().count() >= line as usize
}
pub(crate) fn index_file(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: IndexFileParams = parse_params(params)?;
    let (relative_path, absolute_path) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    if absolute_path.exists() && state.discovery.matches_path(&state.root, &absolute_path) {
        state.sync_path(&absolute_path)?;
    } else {
        state
            .database
            .remove_file_index(&relative_path)
            .map_err(|error| {
                internal_error(error.context("failed to remove file from SQLite index"))
            })?;
    }
    to_value(IndexFileResult {
        file_path: relative_path,
    })
}

use slipbox_core::{
    AppendHeadingAtOutlinePathParams, AppendHeadingParams, AppendHeadingToNodeParams,
    CaptureNodeParams, CaptureTemplateParams, CaptureTemplatePreviewParams, EnsureFileNodeParams,
    EnsureNodeIdParams, ExtractSubtreeParams, RefileRegionParams, RefileSubtreeParams,
    RewriteFileParams, StructuralWriteOperationKind, StructuralWriteResult,
    UpdateNodeMetadataParams,
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

pub(crate) fn refile_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RefileSubtreeParams = parse_params(params)?;
    let source = state.known_anchor(&params.source_node_key, "source node")?;
    let target = state.known_note(&params.target_node_key, "target node")?;
    let outcome = slipbox_write::refile_subtree(&state.root, &source, &target)
        .map_err(|error| internal_error(error.context("failed to refile subtree")))?;
    let node = state.sync_rewrite(&outcome, "refiled node")?;
    let affected_files =
        state.structural_affected_files(&outcome.changed_paths, &outcome.removed_paths)?;
    to_value(state.structural_report(
        StructuralWriteOperationKind::RefileSubtree,
        affected_files,
        Some(StructuralWriteResult::Node {
            node: Box::new(node),
        }),
    )?)
}

pub(crate) fn refile_region(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: RefileRegionParams = parse_params(params)?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let target = state.known_note(&params.target_node_key, "target node")?;
    let (start, end) = params.normalized_range();
    let outcome = slipbox_write::refile_region(&state.root, &relative_path, start, end, &target)
        .map_err(|error| internal_error(error.context("failed to refile region")))?;
    state.sync_region_rewrite(&outcome)?;
    let affected_files =
        state.structural_affected_files(&outcome.changed_paths, &outcome.removed_paths)?;
    to_value(state.structural_report(
        StructuralWriteOperationKind::RefileRegion,
        affected_files,
        None,
    )?)
}

pub(crate) fn extract_subtree(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: ExtractSubtreeParams = parse_params(params)?;
    let source = state.known_anchor(&params.source_node_key, "source node")?;
    let (relative_path, _) = state
        .resolve_index_path(&params.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let outcome = slipbox_write::extract_subtree(&state.root, &source, &relative_path)
        .map_err(|error| internal_error(error.context("failed to extract subtree")))?;
    let node = state.sync_rewrite(&outcome, "extracted node")?;
    let affected_files =
        state.structural_affected_files(&outcome.changed_paths, &outcome.removed_paths)?;
    to_value(state.structural_report(
        StructuralWriteOperationKind::ExtractSubtree,
        affected_files,
        Some(StructuralWriteResult::Node {
            node: Box::new(node),
        }),
    )?)
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
    state.sync_path(&absolute_path)?;
    let node = state.require_note(&outcome.node_key, "promoted file node")?;
    let changed_paths = vec![outcome.absolute_path];
    let affected_files = state.structural_affected_files(&changed_paths, &[])?;
    to_value(state.structural_report(
        StructuralWriteOperationKind::PromoteFile,
        affected_files,
        Some(StructuralWriteResult::Node {
            node: Box::new(node),
        }),
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
    state.sync_path(&absolute_path)?;
    let anchor = state.require_anchor(&outcome.node_key, "demoted file node")?;
    let changed_paths = vec![outcome.absolute_path];
    let affected_files = state.structural_affected_files(&changed_paths, &[])?;
    to_value(state.structural_report(
        StructuralWriteOperationKind::DemoteFile,
        affected_files,
        Some(StructuralWriteResult::Anchor {
            anchor: Box::new(anchor),
        }),
    )?)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use serde_json::json;
    use slipbox_core::{
        NodeKind, StructuralWriteOperationKind, StructuralWriteReport, StructuralWriteResult,
    };
    use slipbox_index::{DiscoveryPolicy, scan_root_with_policy};
    use tempfile::{TempDir, tempdir};

    use super::{
        demote_entire_file, extract_subtree, promote_entire_file, refile_region, refile_subtree,
    };
    use crate::server::state::ServerState;

    fn indexed_state(files: &[(&str, &str)]) -> Result<(TempDir, ServerState)> {
        let workspace = tempdir()?;
        let root = workspace.path().join("notes");
        fs::create_dir_all(&root)?;
        for (relative_path, source) in files {
            write_file(&root, relative_path, source)?;
        }

        let discovery = DiscoveryPolicy::default();
        let mut state = ServerState::new(
            root.clone(),
            workspace.path().join("slipbox.sqlite"),
            Vec::new(),
            discovery.clone(),
        )?;
        let indexed = scan_root_with_policy(&root, &discovery)?;
        state.database.sync_index(&indexed)?;
        Ok((workspace, state))
    }

    fn write_file(root: &Path, relative_path: &str, source: &str) -> Result<()> {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)?;
        Ok(())
    }

    fn structural_report(value: serde_json::Value) -> StructuralWriteReport {
        let report: StructuralWriteReport =
            serde_json::from_value(value).expect("handler should return structural report");
        assert_eq!(report.validation_error(), None);
        report
    }

    #[test]
    fn refile_subtree_returns_structural_report_and_refreshes_changed_files() -> Result<()> {
        let (_workspace, mut state) = indexed_state(&[
            ("source.org", "#+title: Source\n\n* Move Me\nBody\n"),
            (
                "target.org",
                "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n",
            ),
        ])?;

        let report = structural_report(refile_subtree(
            &mut state,
            json!({
                "source_node_key": "heading:source.org:3",
                "target_node_key": "heading:target.org:3"
            }),
        )?);

        assert_eq!(
            report.operation,
            StructuralWriteOperationKind::RefileSubtree
        );
        assert_eq!(
            report.affected_files.changed_files,
            vec!["target.org", "source.org"]
        );
        assert!(report.affected_files.removed_files.is_empty());
        let StructuralWriteResult::Node { node } = report
            .result
            .expect("refile subtree should return moved node")
        else {
            panic!("expected node result");
        };
        assert_eq!(node.title, "Move Me");
        assert_eq!(node.file_path, "target.org");
        assert_eq!(node.level, 2);
        let explicit_id = node.explicit_id.clone().expect("refile should assign ID");
        let refreshed = state
            .database
            .node_from_id(&explicit_id)?
            .expect("moved node should be readable after handler");
        assert_eq!(refreshed.file_path, "target.org");
        assert_eq!(refreshed.level, 2);

        Ok(())
    }

    #[test]
    fn refile_region_returns_removed_files_and_cleans_index() -> Result<()> {
        let (_workspace, mut state) = indexed_state(&[
            ("source.org", "* Move Me\nBody\n"),
            (
                "target.org",
                "#+title: Target\n\n* Parent\n:PROPERTIES:\n:ID: parent-id\n:END:\n",
            ),
        ])?;
        let source = fs::read_to_string(state.root.join("source.org"))?;

        let report = structural_report(refile_region(
            &mut state,
            json!({
                "file_path": "source.org",
                "start": 1,
                "end": source.chars().count() + 1,
                "target_node_key": "heading:target.org:3"
            }),
        )?);

        assert_eq!(report.operation, StructuralWriteOperationKind::RefileRegion);
        assert_eq!(report.affected_files.changed_files, vec!["target.org"]);
        assert_eq!(report.affected_files.removed_files, vec!["source.org"]);
        assert!(report.result.is_none());
        assert!(
            !state
                .database
                .indexed_files()?
                .iter()
                .any(|file| file == "source.org")
        );
        let moved = state
            .database
            .search_anchors("move me", 10, None)?
            .into_iter()
            .find(|anchor| anchor.title == "Move Me")
            .expect("moved region heading should be indexed under target");
        assert_eq!(moved.file_path, "target.org");
        assert_eq!(moved.level, 2);

        Ok(())
    }

    #[test]
    fn extract_subtree_returns_file_node_report_and_refreshes_new_file() -> Result<()> {
        let (_workspace, mut state) = indexed_state(&[(
            "source.org",
            "#+title: Source\n\n* Move Me :tag:\nBody\n** Child\nMore\n",
        )])?;

        let report = structural_report(extract_subtree(
            &mut state,
            json!({
                "source_node_key": "heading:source.org:3",
                "file_path": "moved.org"
            }),
        )?);

        assert_eq!(
            report.operation,
            StructuralWriteOperationKind::ExtractSubtree
        );
        assert_eq!(
            report.affected_files.changed_files,
            vec!["source.org", "moved.org"]
        );
        assert!(report.affected_files.removed_files.is_empty());
        let StructuralWriteResult::Node { node } =
            report.result.expect("extract should return new file node")
        else {
            panic!("expected node result");
        };
        assert_eq!(node.kind, NodeKind::File);
        assert_eq!(node.file_path, "moved.org");
        assert_eq!(node.title, "Move Me");
        let refreshed = state
            .database
            .note_by_key("file:moved.org")?
            .expect("extracted file node should be indexed");
        assert_eq!(refreshed.title, "Move Me");

        Ok(())
    }

    #[test]
    fn promote_file_returns_file_node_report_after_refresh() -> Result<()> {
        let (_workspace, mut state) = indexed_state(&[(
            "note.org",
            "* Note :alpha:\n:PROPERTIES:\n:ID: note-id\n:END:\nBody\n\n** Child\nMore\n",
        )])?;

        let report = structural_report(promote_entire_file(
            &mut state,
            json!({
                "file_path": "note.org"
            }),
        )?);

        assert_eq!(report.operation, StructuralWriteOperationKind::PromoteFile);
        assert_eq!(report.affected_files.changed_files, vec!["note.org"]);
        assert!(report.affected_files.removed_files.is_empty());
        let StructuralWriteResult::Node { node } =
            report.result.expect("promote should return file node")
        else {
            panic!("expected node result");
        };
        assert_eq!(node.kind, NodeKind::File);
        assert_eq!(node.node_key, "file:note.org");
        assert_eq!(node.explicit_id.as_deref(), Some("note-id"));
        assert!(state.database.note_by_key("file:note.org")?.is_some());

        Ok(())
    }

    #[test]
    fn demote_file_returns_anchor_report_without_note_only_collapse() -> Result<()> {
        let (_workspace, mut state) =
            indexed_state(&[("note.org", "#+title: Note\nBody\n\n* Child\nMore\n")])?;

        let report = structural_report(demote_entire_file(
            &mut state,
            json!({
                "file_path": "note.org"
            }),
        )?);

        assert_eq!(report.operation, StructuralWriteOperationKind::DemoteFile);
        assert_eq!(report.affected_files.changed_files, vec!["note.org"]);
        assert!(report.affected_files.removed_files.is_empty());
        let StructuralWriteResult::Anchor { anchor } =
            report.result.expect("demote should return root anchor")
        else {
            panic!("expected anchor result");
        };
        assert_eq!(anchor.kind, NodeKind::Heading);
        assert_eq!(anchor.node_key, "heading:note.org:1");
        assert_eq!(anchor.explicit_id, None);
        assert!(
            state
                .database
                .anchor_by_key("heading:note.org:1")?
                .is_some()
        );
        assert!(state.database.note_by_key("heading:note.org:1")?.is_none());

        Ok(())
    }
}

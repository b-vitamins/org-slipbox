use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use slipbox_core::{
    SlipboxLinkRewriteApplication, SlipboxLinkRewriteAppliedEntry, SlipboxLinkRewriteApplyParams,
    SlipboxLinkRewriteApplyResult, SlipboxLinkRewritePreview, SlipboxLinkRewritePreviewEntry,
    SlipboxLinkRewritePreviewParams, SlipboxLinkRewritePreviewResult,
    StructuralWriteIndexRefreshStatus,
};
use slipbox_rpc::{JsonRpcError, JsonRpcErrorObject};

use crate::server::rpc::{internal_error, parse_params, to_value};
use crate::server::state::ServerState;

pub(crate) fn slipbox_link_rewrite_preview(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SlipboxLinkRewritePreviewParams = parse_params(params)?;
    if let Some(error) = params.validation_error() {
        return Err(invalid_request(error));
    }

    let preview = build_slipbox_link_rewrite_preview(state, &params.file_path)?;
    to_value(SlipboxLinkRewritePreviewResult { preview })
}

pub(crate) fn slipbox_link_rewrite_apply(
    state: &mut ServerState,
    params: serde_json::Value,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: SlipboxLinkRewriteApplyParams = parse_params(params)?;
    if let Some(error) = params.validation_error() {
        return Err(invalid_request(error));
    }

    let (relative_path, absolute_path) = state
        .resolve_index_path(&params.expected_preview.file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    let current_preview = build_slipbox_link_rewrite_preview_for_path(
        state,
        relative_path.clone(),
        absolute_path.clone(),
    )?;
    if current_preview != params.expected_preview {
        return Err(invalid_request(format!(
            "stale slipbox link rewrite preview for {}",
            params.expected_preview.file_path
        )));
    }

    let mut changed_paths = Vec::new();
    let explicit_ids = ensure_slipbox_link_rewrite_target_ids(
        state,
        &params.expected_preview.rewrites,
        &mut changed_paths,
    )?;
    let applied = rewrite_slipbox_links_in_file(
        &absolute_path,
        &params.expected_preview.rewrites,
        &explicit_ids,
    )?;
    changed_paths.push(absolute_path.clone());
    dedup_paths(&mut changed_paths);
    state.sync_path(&absolute_path)?;
    for path in &changed_paths {
        if path != &absolute_path {
            state.sync_path(path)?;
        }
    }
    let affected_files = state.structural_affected_files(&changed_paths, &[])?;
    let application = SlipboxLinkRewriteApplication {
        file_path: relative_path,
        rewrites: applied,
        affected_files,
        index_refresh: StructuralWriteIndexRefreshStatus::Refreshed,
    };
    if let Some(error) = application.validation_error() {
        return Err(internal_error(anyhow!(
            "invalid slipbox link rewrite application: {error}"
        )));
    }

    to_value(SlipboxLinkRewriteApplyResult { application })
}

fn build_slipbox_link_rewrite_preview(
    state: &ServerState,
    file_path: &str,
) -> Result<SlipboxLinkRewritePreview, JsonRpcError> {
    let (relative_path, absolute_path) = state
        .resolve_index_path(file_path)
        .map_err(|error| internal_error(error.context("failed to resolve file path")))?;
    build_slipbox_link_rewrite_preview_for_path(state, relative_path, absolute_path)
}

fn build_slipbox_link_rewrite_preview_for_path(
    state: &ServerState,
    relative_path: String,
    absolute_path: PathBuf,
) -> Result<SlipboxLinkRewritePreview, JsonRpcError> {
    let source = fs::read_to_string(&absolute_path)
        .map_err(|error| internal_error(anyhow!(error).context("failed to read link source")))?;
    let parsed_links = parse_slipbox_links(&source);
    let rewrites = parsed_links
        .into_iter()
        .map(|link| link.into_preview_entry(state, &relative_path))
        .collect::<Result<Vec<_>, _>>()?;
    let preview = SlipboxLinkRewritePreview {
        file_path: relative_path,
        rewrites,
    };
    if let Some(error) = preview.validation_error() {
        return Err(internal_error(anyhow!(
            "invalid slipbox link rewrite preview: {error}"
        )));
    }
    Ok(preview)
}

fn ensure_slipbox_link_rewrite_target_ids(
    state: &mut ServerState,
    entries: &[SlipboxLinkRewritePreviewEntry],
    changed_paths: &mut Vec<PathBuf>,
) -> Result<HashMap<String, String>, JsonRpcError> {
    let mut explicit_ids = HashMap::new();
    let mut ensured_targets = HashSet::new();
    for entry in entries {
        if let Some(explicit_id) = &entry.target_explicit_id {
            explicit_ids.insert(entry.target.node_key.clone(), explicit_id.clone());
            continue;
        }
        if ensured_targets.insert(entry.target.node_key.clone()) {
            let target = entry.target.clone().into();
            let updated_path =
                slipbox_write::ensure_node_id(&state.root, &target).map_err(|error| {
                    internal_error(error.context("failed to assign target node ID"))
                })?;
            changed_paths.push(updated_path.clone());
            state.sync_path(&updated_path)?;
        }
        let updated_target = state.require_anchor(&entry.target.node_key, "updated target node")?;
        let explicit_id = updated_target.explicit_id.ok_or_else(|| {
            internal_error(anyhow!(
                "updated target node {} still has no explicit ID",
                entry.target.node_key
            ))
        })?;
        explicit_ids.insert(entry.target.node_key.clone(), explicit_id);
    }
    Ok(explicit_ids)
}

fn rewrite_slipbox_links_in_file(
    absolute_path: &Path,
    expected_entries: &[SlipboxLinkRewritePreviewEntry],
    explicit_ids: &HashMap<String, String>,
) -> Result<Vec<SlipboxLinkRewriteAppliedEntry>, JsonRpcError> {
    let source = fs::read_to_string(absolute_path)
        .map_err(|error| internal_error(anyhow!(error).context("failed to read link source")))?;
    let parsed_links = parse_slipbox_links(&source);
    let mut replacements = Vec::new();
    let mut applied = Vec::new();
    let mut search_from = 0_usize;

    for expected in expected_entries {
        let Some((parsed_index, parsed)) = parsed_links
            .iter()
            .enumerate()
            .skip(search_from)
            .find(|(_, candidate)| candidate.matches_expected(expected))
        else {
            return Err(invalid_request(format!(
                "stale slipbox link rewrite preview at {}:{}",
                expected.line, expected.column
            )));
        };
        search_from = parsed_index + 1;
        let explicit_id = explicit_ids
            .get(&expected.target.node_key)
            .ok_or_else(|| {
                internal_error(anyhow!(
                    "missing explicit ID for target {}",
                    expected.target.node_key
                ))
            })?
            .clone();
        let replacement = format!("[[id:{}][{}]]", explicit_id, parsed.description);
        replacements.push((parsed.start, parsed.end, replacement.clone()));
        applied.push(SlipboxLinkRewriteAppliedEntry {
            line: parsed.line,
            column: parsed.column,
            title_or_alias: parsed.title_or_alias.clone(),
            target_node_key: expected.target.node_key.clone(),
            target_explicit_id: explicit_id,
            replacement,
        });
    }

    let mut rewritten = source;
    replacements.sort_by_key(|(start, _, _)| *start);
    for (start, end, replacement) in replacements.into_iter().rev() {
        rewritten.replace_range(start..end, &replacement);
    }
    fs::write(absolute_path, rewritten)
        .map_err(|error| internal_error(anyhow!(error).context("failed to write link source")))?;
    Ok(applied)
}

fn dedup_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

#[derive(Debug, Clone)]
struct ParsedSlipboxLink {
    line: u32,
    column: u32,
    preview: String,
    link_text: String,
    title_or_alias: String,
    description: String,
    start: usize,
    end: usize,
}

impl ParsedSlipboxLink {
    fn into_preview_entry(
        self,
        state: &ServerState,
        relative_path: &str,
    ) -> Result<SlipboxLinkRewritePreviewEntry, JsonRpcError> {
        let mut matches = state
            .database
            .node_from_title_or_alias(&self.title_or_alias, false)
            .map_err(|error| internal_error(error.context("failed to resolve slipbox link")))?;
        if matches.is_empty() {
            return Err(invalid_request(format!(
                "unresolved slipbox link target {} at {}:{}:{}",
                self.title_or_alias, relative_path, self.line, self.column
            )));
        }
        if matches.len() > 1 {
            return Err(invalid_request(format!(
                "multiple nodes match slipbox link target {} at {}:{}:{}",
                self.title_or_alias, relative_path, self.line, self.column
            )));
        }
        let target = matches.remove(0);
        let target_explicit_id = target.explicit_id.clone();
        let replacement = target_explicit_id
            .as_ref()
            .map(|explicit_id| format!("[[id:{}][{}]]", explicit_id, self.description));
        Ok(SlipboxLinkRewritePreviewEntry {
            line: self.line,
            column: self.column,
            preview: self.preview,
            link_text: self.link_text,
            title_or_alias: self.title_or_alias,
            description: self.description,
            target,
            target_explicit_id,
            replacement,
        })
    }

    fn matches_expected(&self, expected: &SlipboxLinkRewritePreviewEntry) -> bool {
        self.link_text == expected.link_text
            && self.title_or_alias == expected.title_or_alias
            && self.description == expected.description
    }
}

fn parse_slipbox_links(source: &str) -> Vec<ParsedSlipboxLink> {
    let mut parsed = Vec::new();
    let mut line_start = 0_usize;
    for (line_index, segment) in source.split_inclusive('\n').enumerate() {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        parse_slipbox_links_in_line(line, line_index as u32 + 1, line_start, &mut parsed);
        line_start += segment.len();
    }
    if !source.ends_with('\n') && source.is_empty() {
        parse_slipbox_links_in_line("", 1, 0, &mut parsed);
    }
    parsed
}

fn parse_slipbox_links_in_line(
    line: &str,
    row: u32,
    line_start: usize,
    parsed: &mut Vec<ParsedSlipboxLink>,
) {
    let mut offset = 0_usize;
    while let Some(relative_start) = line[offset..].find("[[") {
        let start = offset + relative_start;
        let suffix = &line[start + 2..];
        let Some(end_inner) = suffix.find("]]") else {
            break;
        };
        let inner = &suffix[..end_inner];
        let (target, label) = inner
            .split_once("][")
            .map_or((inner, None), |(target, label)| (target, Some(label)));
        let target = target.trim();
        if let Some(title_or_alias) = target.strip_prefix("slipbox:").map(str::trim)
            && !title_or_alias.is_empty()
        {
            let description = label
                .filter(|value| !value.is_empty())
                .unwrap_or(title_or_alias)
                .to_owned();
            let end = start + 2 + end_inner + 2;
            parsed.push(ParsedSlipboxLink {
                line: row,
                column: column_number(line, start),
                preview: preview_snippet(line),
                link_text: line[start..end].to_owned(),
                title_or_alias: title_or_alias.to_owned(),
                description,
                start: line_start + start,
                end: line_start + end,
            });
        }
        offset = start + 2 + end_inner + 2;
    }
}

fn column_number(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset].chars().count() as u32 + 1
}

fn preview_snippet(line: &str) -> String {
    line.trim().to_owned()
}

fn invalid_request(message: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(JsonRpcErrorObject::invalid_request(message.into()))
}

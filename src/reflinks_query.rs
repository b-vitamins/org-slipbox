use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use regex::{Regex, RegexBuilder};

use slipbox_core::{NodeRecord, ReflinkRecord};
use slipbox_store::Database;

use crate::text_query::{
    build_node_ranges, column_number, node_range_for_key, source_range_for_row,
};

/// Return structured textual ref occurrences across indexed files.
///
/// Results preserve the dedicated-buffer reflink meaning: fixed-string,
/// case-insensitive matches for the queried node's refs across indexed file
/// contents, with `@citekey` refs also matching `cite:citekey`, and
/// occurrences inside the queried node's own subtree excluded.
///
/// Results are emitted in indexed file-path order, then source row and match
/// column order within each file. Duplicate occurrences are removed and the
/// query is truncated to `limit` in that same order.
pub(crate) fn query_reflinks(
    database: &Database,
    root: &Path,
    node: &NodeRecord,
    limit: usize,
) -> Result<Vec<ReflinkRecord>> {
    let patterns = reflink_patterns(&node.refs);
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let matcher = build_reflink_matcher(&patterns)?;
    let indexed_files = database.indexed_files()?;
    let limit = limit.clamp(1, 1_000);
    let mut results = Vec::new();
    let mut seen = BTreeSet::new();

    for file_path in indexed_files {
        if results.len() >= limit {
            break;
        }

        let absolute_path = root.join(&file_path);
        if !absolute_path.exists() {
            continue;
        }

        let source = slipbox_index::read_source(&absolute_path)
            .with_context(|| format!("failed to read indexed file {}", absolute_path.display()))?;
        let nodes = database
            .nodes_in_file(&file_path)
            .with_context(|| format!("failed to read indexed nodes for {file_path}"))?;
        if nodes.is_empty() {
            continue;
        }

        let ranges = build_node_ranges(&nodes, source.lines().count().max(1) as u32);
        let current_range = if file_path == node.file_path {
            Some(node_range_for_key(&ranges, &node.node_key).ok_or_else(|| {
                anyhow!(
                    "queried node {} was not found in indexed file {}",
                    node.node_key,
                    file_path
                )
            })?)
        } else {
            None
        };
        let mut source_index = 0_usize;

        for (row_index, line) in source.lines().enumerate() {
            if results.len() >= limit {
                break;
            }

            let row = row_index as u32 + 1;
            if current_range.is_some_and(|(start, end)| start <= row && row <= end) {
                continue;
            }

            let Some(source_range) = source_range_for_row(&ranges, row, &mut source_index) else {
                continue;
            };

            for matched in matcher.find_iter(line) {
                let result = ReflinkRecord {
                    source_node: source_range.node.clone(),
                    row,
                    col: column_number(line, matched.start()),
                    preview: line.trim_end().to_owned(),
                    matched_reference: matched.as_str().to_owned(),
                };
                let key = (
                    result.source_node.node_key.clone(),
                    result.row,
                    result.col,
                    result.matched_reference.clone(),
                );
                if seen.insert(key) {
                    results.push(result);
                }
                if results.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(results)
}
fn reflink_patterns(refs: &[String]) -> Vec<String> {
    let mut patterns = Vec::new();

    for reference in refs {
        let trimmed = reference.trim();
        if trimmed.is_empty() {
            continue;
        }

        patterns.push(trimmed.to_owned());
        if let Some(cite_key) = trimmed.strip_prefix('@') {
            patterns.push(format!("cite:{cite_key}"));
        }
    }

    patterns.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    patterns.dedup();
    patterns
}

fn build_reflink_matcher(patterns: &[String]) -> Result<Regex> {
    let expression = patterns
        .iter()
        .map(|pattern| regex::escape(pattern))
        .collect::<Vec<_>>()
        .join("|");
    RegexBuilder::new(&expression)
        .case_insensitive(true)
        .build()
        .context("failed to build reflink matcher")
}

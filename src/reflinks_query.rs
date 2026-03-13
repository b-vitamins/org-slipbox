use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use regex::{Regex, RegexBuilder};

use slipbox_core::{NodeKind, NodeRecord, ReflinkRecord};
use slipbox_store::Database;

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

#[derive(Debug, Clone)]
struct NodeRange {
    node: NodeRecord,
    start_line: u32,
    end_line: u32,
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

fn build_node_ranges(nodes: &[NodeRecord], total_lines: u32) -> Vec<NodeRange> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| NodeRange {
            node: node.clone(),
            start_line: node.line.max(1),
            end_line: node_end_line(nodes, index, total_lines),
        })
        .collect()
}

fn node_end_line(nodes: &[NodeRecord], index: usize, total_lines: u32) -> u32 {
    let node = &nodes[index];
    if node.kind == NodeKind::File {
        return total_lines.max(node.line);
    }

    for candidate in &nodes[index + 1..] {
        if candidate.line > node.line && candidate.level <= node.level {
            return candidate.line.saturating_sub(1).max(node.line);
        }
    }

    total_lines.max(node.line)
}

fn node_range_for_key(ranges: &[NodeRange], node_key: &str) -> Option<(u32, u32)> {
    ranges
        .iter()
        .find(|range| range.node.node_key == node_key)
        .map(|range| (range.start_line, range.end_line))
}

fn source_range_for_row<'a>(
    ranges: &'a [NodeRange],
    row: u32,
    source_index: &mut usize,
) -> Option<&'a NodeRange> {
    while *source_index + 1 < ranges.len() && ranges[*source_index + 1].start_line <= row {
        *source_index += 1;
    }

    let mut index = (*source_index).min(ranges.len().saturating_sub(1));
    loop {
        let range = &ranges[index];
        if range.start_line <= row && row <= range.end_line {
            return Some(range);
        }
        if index == 0 {
            return None;
        }
        index -= 1;
    }
}

fn column_number(line: &str, byte_offset: usize) -> u32 {
    line[..byte_offset].chars().count() as u32 + 1
}

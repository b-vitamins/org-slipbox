use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use regex::{Regex, RegexBuilder};

use slipbox_core::{AnchorRecord, IndexedLink, UnlinkedReferenceRecord};
use slipbox_store::Database;

use crate::text_query::{
    build_structural_ranges, byte_offset_for_column, column_number, has_phrase_boundaries,
    structural_range_for_key, structural_range_for_row,
};

/// Return structured title and alias mentions that are not already linked.
///
/// Matches are case-insensitive fixed-string phrase occurrences for the queried
/// node's title and aliases. A phrase counts only when the characters
/// immediately before and after the match are either absent or not
/// alphanumeric/underscore characters.
///
/// Results exclude occurrences inside the queried node's own subtree and
/// occurrences already covered by an indexed `id:` link to the same node. They
/// are emitted in indexed file-path order, then row and column order within
/// each file. Duplicate occurrences are removed and truncation to `limit`
/// happens in that same order.
pub(crate) fn query_unlinked_references(
    database: &Database,
    root: &Path,
    node: &AnchorRecord,
    limit: usize,
) -> Result<Vec<UnlinkedReferenceRecord>> {
    let patterns = mention_patterns(node);
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let matcher = build_mention_matcher(&patterns)?;
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
        let lines = source.lines().collect::<Vec<_>>();
        let visible_anchors = database
            .anchors_in_file(&file_path)
            .with_context(|| format!("failed to read indexed anchors for {file_path}"))?;
        if visible_anchors.is_empty() {
            continue;
        }
        let anchors_by_key = visible_anchors
            .iter()
            .map(|candidate| (candidate.node_key.as_str(), candidate))
            .collect::<HashMap<_, _>>();
        let outline = slipbox_index::scan_source_outline(&file_path, &source);

        let ranges = build_structural_ranges(&outline, lines.len().max(1) as u32);
        let current_range = if file_path == node.file_path {
            Some(
                structural_range_for_key(&ranges, &node.node_key).ok_or_else(|| {
                    anyhow!(
                        "queried node {} was not found in indexed file {}",
                        node.node_key,
                        file_path
                    )
                })?,
            )
        } else {
            None
        };
        let linked_spans = linked_spans_by_row(database, &file_path, &lines, node)?;
        let mut source_index = 0_usize;

        for (row_index, line) in lines.iter().enumerate() {
            if results.len() >= limit {
                break;
            }

            let row = row_index as u32 + 1;
            if current_range.is_some_and(|(start, end)| start <= row && row <= end) {
                continue;
            }

            let Some(source_range) = structural_range_for_row(&ranges, row, &mut source_index)
            else {
                continue;
            };
            if source_range.excluded {
                continue;
            }
            let source_anchor = anchors_by_key
                .get(source_range.node_key.as_str())
                .ok_or_else(|| {
                    anyhow!(
                        "indexed source anchor {} was not found in visible anchor map for {}",
                        source_range.node_key,
                        file_path
                    )
                })?;
            let covered_spans = linked_spans.get(&row);

            for matched in matcher.find_iter(line) {
                let start = matched.start();
                let end = matched.end();
                if !has_phrase_boundaries(line, start, end) {
                    continue;
                }
                if covered_spans.is_some_and(|spans| span_is_linked(start, end, spans)) {
                    continue;
                }

                let result = UnlinkedReferenceRecord {
                    source_anchor: (*source_anchor).clone(),
                    row,
                    col: column_number(line, start),
                    preview: line.trim_end().to_owned(),
                    matched_text: matched.as_str().to_owned(),
                };
                let key = (
                    result.source_anchor.node_key.clone(),
                    result.row,
                    result.col,
                    result.matched_text.clone(),
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

fn mention_patterns(node: &AnchorRecord) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut patterns = Vec::new();

    for candidate in
        std::iter::once(node.title.as_str()).chain(node.aliases.iter().map(String::as_str))
    {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            continue;
        }

        if seen.insert(trimmed.to_lowercase()) {
            patterns.push(trimmed.to_owned());
        }
    }

    patterns.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| left.to_lowercase().cmp(&right.to_lowercase()))
            .then_with(|| left.cmp(right))
    });
    patterns
}

fn build_mention_matcher(patterns: &[String]) -> Result<Regex> {
    let expression = patterns
        .iter()
        .map(|pattern| regex::escape(pattern))
        .collect::<Vec<_>>()
        .join("|");
    RegexBuilder::new(&expression)
        .case_insensitive(true)
        .build()
        .context("failed to build unlinked-reference matcher")
}

fn linked_spans_by_row(
    database: &Database,
    file_path: &str,
    lines: &[&str],
    node: &AnchorRecord,
) -> Result<BTreeMap<u32, Vec<(usize, usize)>>> {
    let Some(explicit_id) = node.explicit_id.as_deref() else {
        return Ok(BTreeMap::new());
    };

    let links = database
        .links_to_destination_in_file(file_path, explicit_id)
        .with_context(|| format!("failed to read indexed links for {file_path}"))?;
    if links.is_empty() {
        return Ok(BTreeMap::new());
    }

    let mut spans_by_row = BTreeMap::<u32, Vec<(usize, usize)>>::new();
    for link in links {
        let Some(line) = lines.get(link.line.saturating_sub(1) as usize) else {
            continue;
        };
        if let Some(span) = linked_label_span(line, &link) {
            spans_by_row.entry(link.line).or_default().push(span);
        }
    }

    for spans in spans_by_row.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }

    Ok(spans_by_row)
}

fn linked_label_span(line: &str, link: &IndexedLink) -> Option<(usize, usize)> {
    let start = byte_offset_for_column(line, link.column)?;
    let suffix = line.get(start + 2..)?;
    if !line[start..].starts_with("[[") {
        return None;
    }

    let end = suffix.find("]]")?;
    let inner = &suffix[..end];
    let (path, label) = inner.split_once("][")?;
    let destination_id = path.trim().strip_prefix("id:")?.trim();
    if destination_id != link.destination_explicit_id {
        return None;
    }

    let label_start = start + 2 + path.len() + 2;
    let label_end = label_start + label.len();
    Some((label_start, label_end))
}

fn span_is_linked(start: usize, end: usize, linked_spans: &[(usize, usize)]) -> bool {
    linked_spans
        .iter()
        .any(|(linked_start, linked_end)| *linked_start <= start && end <= *linked_end)
}

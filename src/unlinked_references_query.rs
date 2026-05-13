use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use regex::{Regex, RegexBuilder};

use slipbox_core::{AnchorRecord, ExplorationExplanation, IndexedLink, UnlinkedReferenceRecord};
use slipbox_store::Database;

use crate::text_query::{
    build_structural_ranges, byte_offset_for_column, column_number, has_phrase_boundaries,
    structural_range_for_key,
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
    let indexed_files = candidate_file_paths(database, &patterns)?;
    let links_by_file = if let Some(explicit_id) = node.explicit_id.as_deref() {
        database
            .links_to_destination_by_file(explicit_id)
            .context("failed to read indexed links for unlinked-reference scan")?
    } else {
        HashMap::new()
    };
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

        let Some(candidate) = database.occurrence_document(&file_path)? else {
            continue;
        };
        let visible_anchors = database
            .anchors_in_file(&file_path)
            .with_context(|| format!("failed to read indexed anchors for {file_path}"))?;
        if visible_anchors.is_empty() {
            continue;
        }

        let current_range = if file_path == node.file_path {
            Some(current_subtree_range(root, &file_path, node)?)
        } else {
            None
        };
        let lines = candidate.search_text.lines().collect::<Vec<_>>();
        let lines_by_row = candidate
            .line_rows
            .iter()
            .copied()
            .zip(lines.iter().copied())
            .collect::<HashMap<_, _>>();
        let linked_spans = linked_spans_by_row(
            links_by_file
                .get(&file_path)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &lines_by_row,
        );
        let mut source_anchor_index = 0_usize;

        for (line_index, line) in lines.iter().enumerate() {
            if results.len() >= limit {
                break;
            }

            let row = candidate.line_rows[line_index];
            if current_range.is_some_and(|(start, end)| start <= row && row <= end) {
                continue;
            }

            let source_anchor =
                resolve_owning_anchor(&visible_anchors, row, &mut source_anchor_index);
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
                    source_anchor: source_anchor.clone(),
                    row,
                    col: column_number(line, start),
                    preview: line.trim_end().to_owned(),
                    matched_text: matched.as_str().to_owned(),
                    explanation: ExplorationExplanation::UnlinkedReference {
                        matched_text: matched.as_str().to_owned(),
                    },
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

fn candidate_file_paths(database: &Database, patterns: &[String]) -> Result<Vec<String>> {
    if patterns
        .iter()
        .any(|pattern| pattern.trim().chars().count() < 3)
    {
        return database.indexed_files();
    }

    let mut candidates = BTreeSet::new();
    for pattern in patterns {
        let mut offset = 0_usize;
        loop {
            let batch = database
                .search_occurrence_document_paths(pattern, 1_000, offset)
                .with_context(|| {
                    format!("failed to prefilter unlinked-reference files for {pattern:?}")
                })?;
            let batch_len = batch.len();
            if batch_len == 0 {
                break;
            }
            candidates.extend(batch);
            offset += batch_len;
            if batch_len < 1_000 {
                break;
            }
        }
    }

    Ok(candidates.into_iter().collect())
}

fn current_subtree_range(root: &Path, file_path: &str, node: &AnchorRecord) -> Result<(u32, u32)> {
    let absolute_path = root.join(file_path);
    if !absolute_path.exists() {
        return Ok((0, 0));
    }

    let source = slipbox_index::read_source(&absolute_path)
        .with_context(|| format!("failed to read indexed file {}", absolute_path.display()))?;
    let line_count = source.lines().count().max(1) as u32;
    let outline = slipbox_index::scan_source_outline(file_path, &source);
    let ranges = build_structural_ranges(&outline, line_count);
    structural_range_for_key(&ranges, &node.node_key).ok_or_else(|| {
        anyhow!(
            "queried node {} was not found in indexed file {}",
            node.node_key,
            file_path
        )
    })
}

fn resolve_owning_anchor<'a>(
    anchors: &'a [AnchorRecord],
    row: u32,
    index: &mut usize,
) -> &'a AnchorRecord {
    while *index + 1 < anchors.len() && anchors[*index + 1].line <= row {
        *index += 1;
    }
    &anchors[*index]
}

fn linked_spans_by_row(
    links: &[IndexedLink],
    lines_by_row: &HashMap<u32, &str>,
) -> BTreeMap<u32, Vec<(usize, usize)>> {
    if links.is_empty() {
        return BTreeMap::new();
    }

    let mut spans_by_row = BTreeMap::<u32, Vec<(usize, usize)>>::new();
    for link in links {
        let Some(line) = lines_by_row.get(&link.line).copied() else {
            continue;
        };
        if let Some(span) = linked_label_span(line, link) {
            spans_by_row.entry(link.line).or_default().push(span);
        }
    }

    for spans in spans_by_row.values_mut() {
        spans.sort_unstable();
        spans.dedup();
    }

    spans_by_row
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

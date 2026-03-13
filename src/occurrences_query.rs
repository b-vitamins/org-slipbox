use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};

use slipbox_core::OccurrenceRecord;
use slipbox_store::Database;

use crate::text_query::column_number;

/// Return structured fixed-string text occurrences across indexed visible files.
///
/// Candidate files are narrowed through the derived SQLite index, then exact
/// case-insensitive literal matching and preview generation happen in Rust over
/// the visible indexed lines in each file. Owning nodes are resolved from the
/// indexed outline in row order. Results are emitted in indexed file-path
/// order, then row and column order within each file, and truncation to
/// `limit` happens in that same order. Queries shorter than 3 characters do
/// not return results so the interactive path stays fully index-backed.
pub(crate) fn query_occurrences(
    database: &Database,
    query: &str,
    limit: usize,
) -> Result<Vec<OccurrenceRecord>> {
    let Some(matcher) = build_occurrence_matcher(query)? else {
        return Ok(Vec::new());
    };

    let limit = limit.clamp(1, 1_000);
    let batch_size = limit.clamp(8, 32);
    let mut results = Vec::new();
    let mut offset = 0_usize;

    loop {
        let candidate_file_paths =
            database.search_occurrence_document_paths(query, batch_size, offset)?;
        let candidate_count = candidate_file_paths.len();
        if candidate_count == 0 {
            break;
        }
        offset += candidate_count;
        let owning_nodes_by_file = database.nodes_in_files(&candidate_file_paths)?;

        for file_path in candidate_file_paths {
            let Some(candidate) = database.occurrence_document(&file_path)? else {
                continue;
            };
            let Some(owning_nodes) = owning_nodes_by_file.get(&candidate.file_path) else {
                continue;
            };
            let mut owning_node_index = 0_usize;
            let lines = candidate.search_text.lines().collect::<Vec<_>>();
            let line_starts = line_start_offsets(&lines);
            let mut line_index = 0_usize;
            for matched in matcher.find_iter(&candidate.search_text) {
                while line_index + 1 < line_starts.len()
                    && line_starts[line_index + 1] <= matched.start()
                {
                    line_index += 1;
                }
                let line = lines[line_index];
                let row = candidate.line_rows[line_index];
                let owning_node =
                    resolve_owning_node(owning_nodes, row, &mut owning_node_index).clone();
                results.push(OccurrenceRecord {
                    file_path: candidate.file_path.clone(),
                    row,
                    col: column_number(line, matched.start() - line_starts[line_index]),
                    preview: line.trim_end().to_owned(),
                    matched_text: matched.as_str().to_owned(),
                    owning_node: Some(owning_node),
                });
                if results.len() >= limit {
                    return Ok(results);
                }
            }
        }

        if candidate_count < batch_size {
            break;
        }
    }

    Ok(results)
}

fn line_start_offsets(lines: &[&str]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(lines.len());
    let mut offset = 0_usize;
    for line in lines {
        starts.push(offset);
        offset += line.len() + 1;
    }
    starts
}

fn resolve_owning_node<'a>(
    nodes: &'a [slipbox_core::NodeRecord],
    row: u32,
    index: &mut usize,
) -> &'a slipbox_core::NodeRecord {
    while *index + 1 < nodes.len() && nodes[*index + 1].line <= row {
        *index += 1;
    }
    &nodes[*index]
}

fn build_occurrence_matcher(query: &str) -> Result<Option<Regex>> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(
        RegexBuilder::new(&regex::escape(trimmed))
            .case_insensitive(true)
            .build()
            .context("failed to build occurrence matcher")?,
    ))
}
